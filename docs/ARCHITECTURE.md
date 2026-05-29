# QRacer 架构文档

> 文档版本：v1.0  
> 最后更新：2026-05-28  
> 项目状态：阶段 2（预处理 + QR 定位 + 透视校正）已完成

---

## 1. 项目背景与目标

### 1.1 业务背景
QRacer 服务于柔性版印刷（柔印）印前工作。印前设计师常常需要把客户提供的**二维码截图**（来自微信、抖音、网页等）转成可用于制版的矢量文件（SVG）。这个过程必须保证**像素级几何精度**——即"扫码结果一致"并不够，**码点位与原图必须 1:1 还原**。

### 1.2 工具流程
```
导入图片 → 识别码类型 → 矢量化还原 → 导出 SVG / 复制到剪贴板（粘贴进 Illustrator）
```

### 1.3 支持的码类型
| 码类型 | 几何形状 | 协议公开 | 矢量化策略 |
|---|---|---|---|
| 二维码（QR） | 方阵 | ✅ 公开（ISO/IEC 18004） | 主：解码→8 掩膜重生成；兜：网格采样 |
| 微信小程序码 | 圆形放射 | ❌ 私有 | 仅网格采样 |
| 抖音码 | 同心圆 | ❌ 私有 | 仅网格采样 |

### 1.4 核心约束
1. **几何精度优先**：码点位必须与原图 1:1，扫码一致≠合格
2. **用户可校验**：左右对比 + 差异高亮，让用户人眼确认
3. **印前工作流**：输出 SVG（文件）和 EMF（剪贴板粘贴进 AI）
4. **仅 Windows**：暂不考虑跨平台
5. **轻量分发**：单 .exe 文件，目标 release < 20MB

---

## 2. 技术栈

### 2.1 选型表
| 维度 | 选型 | 说明 |
|---|---|---|
| 语言 | Rust 2024 edition | 性能、单二进制、内存安全 |
| GUI | `eframe` + `egui` 0.32 | immediate-mode；OpenGL 后端（`glow`）；启动时注入 Windows CJK 字体 |
| 图像 IO | `image` 0.25 | PNG/JPG/BMP/WebP |
| 图像处理 | `imageproc` 0.25 | 二值化、轮廓、形态学 |
| QR 解码 | `rxing` 0.7 | ZXing 移植；多格式 |
| QR 生成 | `qrcodegen` 1.8 | **关键**：`encode_segments_advanced` 支持 `Some(Mask::new(0..=7))` |
| 矩阵运算 | `nalgebra` 0.33 | 单应矩阵、透视变换 |
| SVG 输出 | 字符串构建 | 直接拼字符串最稳，可控 |
| 剪贴板（图像读） | `arboard` 3 | 跨平台读 |
| 粘贴快捷键 | `windows-sys` 0.60 | Windows 前台 `Ctrl+V`/`Shift+Insert` 物理按键状态检测，用于图片剪贴板 |
| 剪贴板（EMF 写） | `clipboard-win` 5 + `windows` 0.59 | GDI `CreateEnhMetaFile` + `CF_ENHMETAFILE` |
| 文件对话框 | `rfd` 0.15 | Native dialog |
| 错误处理 | `anyhow` 1 + `thiserror` 2 | 应用级用 `anyhow`；库层错误类型用 `thiserror` |

### 2.2 关键技术决策

#### 决策 1：immediate-mode GUI（egui）而非 retained-mode
- **背景**：备选 slint / iced / Tauri
- **选择 egui 的原因**：
  - 单一 Rust crate，无 DSL / WebView 依赖
  - 工具型 UI（按钮、对比、参数面板）非常契合 immediate-mode
  - 二进制体积小（release ~10MB）
- **代价**：每帧重画，需自己缓存重计算结果（如纹理）

#### 决策 2：小程序码/抖音码"仅几何还原，不逆向解码"
- **原因**：协议未公开，逆向后即使能还原内容，未来版本更新会失效
- **取舍**：放弃"标准化重生成"，但避免了维护私有协议解析的负担

#### 决策 3：纯 Rust 图像处理，不依赖 OpenCV
- **原因**：`opencv` crate 需本地编译 OpenCV，破坏单文件分发目标
- **代价**：`imageproc` 没有 Hough Circle，抖音码同心圆定位需自实现（轮廓 + 圆度判定）

#### 决策 4：剪贴板用经典 EMF 而非 EMF+
- **原因**：旧版 Illustrator（CS5 等）对 EMF+ 渲染有问题，经典 EMF 兼容性最好
- **代价**：经典 EMF 不支持半透明色（对二维码来说不需要，黑白）

---

## 3. 整体架构

### 3.1 数据流
```
┌──────────────────────────────────────────────────────────────────┐
│                       QRacerApp (egui)                            │
│                                                                   │
│  ┌──────────┐                                       ┌──────────┐ │
│  │ 工具栏   │                                       │  状态栏  │ │
│  └────┬─────┘                                       └──────────┘ │
│       │ 粘贴/打开                                                 │
│       ▼                                                           │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │  image_io: read_clipboard / open_dialog                  │    │
│  │     → image::DynamicImage                                │    │
│  └────────────────────┬─────────────────────────────────────┘    │
│                       │                                           │
│                       ▼                                           │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │ pipeline::preprocess                                     │    │
│  │   灰度 → Otsu 二值化 → 噪点清理 (binary image)            │    │
│  └────────────────────┬─────────────────────────────────────┘    │
│                       │                                           │
│                       ▼                                           │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │ detect: 自动识别码类型                                    │    │
│  │   ├─ 方形 1:1:3:1:1 嵌套 → CodeKind::Qr                  │    │
│  │   ├─ 圆形嵌套 + 放射纹理 → CodeKind::WxMiniprogram        │    │
│  │   └─ 同心圆 → CodeKind::Douyin                           │    │
│  └────────────────────┬─────────────────────────────────────┘    │
│                       │                                           │
│         ┌─────────────┼─────────────┐                             │
│         ▼             ▼             ▼                             │
│  ┌──────────┐  ┌──────────┐  ┌──────────┐                         │
│  │ codec/qr │  │codec/wx_ │  │codec/dy_ │                         │
│  │          │  │  grid    │  │  grid    │                         │
│  │主:rxing→ │  │定位三牛眼│  │定位3同心 │                         │
│  │qrcodegen │  │→径向网格 │  │→环形网格 │                         │
│  │8 掩膜    │  │→采样     │  │→采样     │                         │
│  │兜:网格   │  │          │  │          │                         │
│  └────┬─────┘  └────┬─────┘  └────┬─────┘                         │
│       │             │             │                               │
│       └─────────────┼─────────────┘                               │
│                     ▼                                             │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │ vector: SVG 字符串构建（rect / path / circle）            │    │
│  └────────────────────┬─────────────────────────────────────┘    │
│                       │                                           │
│         ┌─────────────┴─────────────┐                             │
│         ▼                           ▼                             │
│  ┌──────────────┐           ┌──────────────────┐                  │
│  │ 文件: 写 SVG │           │ clipboard::emf   │                  │
│  │              │           │ SVG → GDI 重放   │                  │
│  │              │           │ → CF_ENHMETAFILE │                  │
│  └──────────────┘           └──────────────────┘                  │
│                                                                   │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │ ui::compare_view: 左右对比 + 差异高亮（XOR 叠加）          │    │
│  └──────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────┘
```

### 3.2 模块职责

```
src/
├── main.rs                   eframe 启动；只做窗口配置
├── app.rs                    QRacerApp：全局状态 + update() 主循环；粘贴快捷键处理
├── code_kind.rs              CodeKind 枚举
├── image_io.rs               剪贴板读图、文件打开、egui 纹理转换
│
├── ui/                       UI 层（egui Widget 描述）
│   ├── mod.rs
│   ├── toolbar.rs            顶部工具栏
│   ├── compare_view.rs       左右对比 + 差异高亮
│   └── mask_panel.rs         (阶段 3) 8 掩膜单选 + 网格兜底按钮
│
├── detect/                   定位与码类型判定
│   ├── mod.rs                自动判定入口 detect_kind()
│   ├── finder_qr.rs          QR 三个 finder pattern (1:1:3:1:1)
│   ├── finder_wx.rs          小程序码三牛眼
│   └── finder_dy.rs          抖音码三同心圆
│
├── pipeline/                 几何预处理 + 网格推算
│   ├── mod.rs
│   ├── preprocess.rs         灰度 → Otsu 二值化 → 形态学清理
│   ├── perspective.rs        从 finder 推单应矩阵 + warp
│   └── grid.rs               推算模块/线/环的几何网格
│
├── codec/                    各码的编解码 / 网格采样
│   ├── mod.rs
│   ├── qr.rs                 主路线：rxing 解码 + qrcodegen 8 掩膜重生成
│   ├── qr_grid.rs            兜底：QR 网格采样
│   ├── wx_grid.rs            小程序码：径向网格采样
│   └── dy_grid.rs            抖音码：同心圆采样
│
├── vector/                   SVG 输出
│   ├── mod.rs
│   ├── svg.rs                SVG 构建器
│   ├── shapes.rs             几何图元：方块、扇形、圆环段
│   └── diff.rs               生成与原图的差异掩膜（用于差异高亮）
│
├── clipboard/                剪贴板写入
│   ├── mod.rs
│   └── emf.rs                经典 EMF 生成 + CF_ENHMETAFILE 写入
│
└── job/                      (阶段 3 引入) 后台任务运行器
    ├── mod.rs
    └── runner.rs             std::sync::mpsc + 线程，避免阻塞 UI
```

---

## 4. 关键数据结构

### 4.1 应用状态 `QRacerApp`
```rust
pub struct QRacerApp {
    pub original: Option<LoadedImage>,     // 用户输入的原图
    pub preview:  Option<LoadedImage>,     // 当前预览；阶段 2 为透视校正二值图，阶段 3+ 为矢量光栅化结果
    pub code_kind: CodeKind,               // 自动识别结果
    pub status: String,                    // 状态栏显示文字

    // 阶段 2+ 增加：
    pub binary: Option<BinaryImage>,       // 二值化中间结果
    pub finders: Option<Vec<QrFinder>>,    // QR finder 候选集合
    pub warped: Option<BinaryImage>,       // 透视校正后的固定尺寸二值图

    // 阶段 3+ 增加：
    pub mask_choice: MaskChoice,           // 当前选用的掩膜 0-7 或网格兜底
    pub last_svg: Option<String>,          // 最近生成的 SVG 文本
    pub diff_overlay: Option<egui::ColorImage>, // 差异叠加层
    pub job_runner: JobRunner,             // 后台任务（识别/重生成）
}
```

### 4.2 几何中间表示
```rust
/// 二值图（true = 前景/黑模块）
pub struct BinaryImage {
    pub w: u32,
    pub h: u32,
    pub data: Vec<bool>,    // 或 BitVec 节省内存
}

/// 一个定位点（中心 + 推算的模块半径）
pub struct Finder {
    pub center: (f64, f64),   // 亚像素中心
    pub module_size: f64,     // 推算的单模块边长（QR）或环宽（DY）
}

/// 三个定位点（QR/小程序码/抖音码都需要 3 个）
pub struct FinderSet {
    pub top_left: Finder,
    pub top_right: Finder,
    pub bottom_left: Finder,
    pub kind_hint: CodeKind,
}

/// QR 网格：把校正后图像分成 NxN 模块
pub struct QrGrid {
    pub version: u8,         // 1-40
    pub size: u32,           // (version-1)*4 + 21
    pub modules: Vec<bool>,  // size*size
}

/// 小程序码网格：极坐标
pub struct WxGrid {
    pub center: (f64, f64),
    pub r_min: f64,           // 第 1 点（最内）半径
    pub r_max: f64,           // 第 13 点（最外）半径
    pub lines: u32,           // 36 / 54 / 72
    pub points_per_line: u32, // 固定 13
    pub samples: Vec<bool>,   // lines * 13
}

/// 抖音码网格
pub struct DyGrid {
    pub center: (f64, f64),
    pub rings: Vec<RingSpec>, // 每环的 r_inner/r_outer/是否装饰环
    pub points_per_ring: u32, // 72 或 120
    pub samples: Vec<bool>,
}

pub struct RingSpec {
    pub r_inner: f64,
    pub r_outer: f64,
    pub is_decoration: bool,  // true 表示装饰环，不参与编码
}
```

---

## 5. 关键算法

### 5.1 QR Finder Pattern 检测
**目标**：在二值图中找到三个 "1:1:3:1:1" 嵌套方块。

```
扫描线法：
1. 对每行执行 RLE（行程编码），得到 [黑长, 白长, 黑长, 白长, 黑长]
2. 检测连续 5 段是否满足 1:1:3:1:1（容差 ±50%）
3. 对每个候选段中心做列方向同样验证
4. 满足两轴验证的候选 = finder 中心
5. 从候选中筛选模块尺寸接近、近似右角、面积较大且 finder 间 timing pattern 黑白交替合理的三点组合，作为 QR 三角
```

### 5.2 透视校正
**目标**：把任意角度截图的码摆正到正方形坐标系。

```
1. 三个 finder 中心 + 推算第四个角（向量加法）
2. V2+ QR 优先搜索右下 alignment pattern，使用三 finder + alignment pattern 组成 4 点透视约束；V1 或未命中时回退三 finder 仿射估计
3. 构造源四角 → 目标 (0,0)~(N,N) 的单应矩阵 H（nalgebra）
4. 用反向映射 + 双线性插值 warp 到目标图
```

### 5.3 QR 网格采样兜底
**目标**：不解码，直接在校正后图上采样每个模块。

```
1. 已知 QR 版本 → size = (version-1)*4 + 21
2. 每模块边长 = 目标图边长 / size
3. 对每个 (i, j)：
   - 取 3x3 中心子区域，多数投票（>=5 黑即判黑）
   - 写入 modules[i*size + j]
4. 输出：SVG 由 size*size 个 rect 拼成
```

### 5.4 小程序码径向采样
```
1. 三个牛眼 → 推圆心 C 和外径 R
2. 元信息区在第 4 点位置（统一坐标）→ 先解元信息
   - 元信息编码版本（36/54/72 线）
3. 已知 lines、每线 13 点：
   - 角度步进 = 360° / lines
   - 半径步进 = (R_max - R_min) / 12
4. 在每个 (line, point) 极坐标采样
5. 输出：每个点画扇形（圆心角 = 角度步进/2，半径 = r±模块半径/2）
```

### 5.5 抖音码同心圆采样
```
1. 三个小同心圆 + 右上大圆 → 求圆心 C
2. 检测环数：从圆心向外扫描径向亮度变化，统计环数 5/6/7
3. 检测每环点数：第 2 环（编码环）做角度扫描 RLE → 72 或 120
4. 对每个 (ring, point) 采样（跳过装饰环：环 1、环 3）
5. 输出：编码环上每个采样点画扇环段
```

### 5.6 差异高亮
```
1. 把生成的 SVG 用 resvg / 内部光栅化器渲染成与原图同尺寸的位图
2. 二值化后与原图二值像素逐位 XOR
3. XOR=1 的像素叠加在原图预览侧，用半透明红色显示
4. 状态栏显示差异像素数 / 模块数
```

---

## 6. 错误处理

### 6.1 分层策略
- **库层（detect / pipeline / codec / vector / clipboard）**：返回 `Result<T, QRacerError>`（用 `thiserror`）
- **应用层（app / ui）**：用 `anyhow::Result`，错误转字符串写入 `status` 字段
- **panic 哲学**：只在真正违反不变式时 panic（如 finder 数组长度不为 3），其他都返回 Err

### 6.2 关键错误类型（阶段 2 已引入）
```rust
#[derive(thiserror::Error, Debug)]
pub enum QRacerError {
    #[error("找不到三个定位点（找到 {0} 个）")]
    InsufficientFinders(usize),
    #[error("无法判定码类型")]
    UnknownCodeKind,
    #[error("QR 解码失败：{0}")]
    QrDecode(String),
    #[error("剪贴板访问失败：{0}")]
    Clipboard(String),
    #[error("IO 错误：{0}")]
    Io(#[from] std::io::Error),
}
```

---

## 7. 并发模型

### 7.1 阶段 1：单线程
所有操作（粘贴、打开、显示）在 UI 线程同步完成。粘贴/打开几十毫秒，可接受。

### 7.2 阶段 3+ 引入后台任务
**触发场景**：识别 + 8 掩膜对比，可能耗时数秒。

```rust
pub struct JobRunner {
    sender: mpsc::Sender<Job>,
    receiver: mpsc::Receiver<JobResult>,
    _handle: std::thread::JoinHandle<()>,
}

pub enum Job {
    Detect { image: DynamicImage },
    GenerateWithMask { mask: u8 },
    GridFallback,
}

pub enum JobResult {
    Detected { kind: CodeKind, finders: FinderSet, homography: Matrix3<f64> },
    Generated { svg: String, raster: ColorImage, diff_count: u32 },
}
```

UI 线程每帧 `try_recv()` 拉结果，不阻塞。

---

## 8. 测试策略

### 8.1 单元测试
- `detect/finder_qr.rs`：用合成 QR 图（`qrcodegen` 生成）验证 finder 检测
- `pipeline/preprocess.rs`：Otsu 阈值在已知样本上的稳定性
- `codec/qr.rs`：解码再生成的双向闭环（不带掩膜重选）

### 8.2 集成测试 / fixture
`tests/fixtures/` 存放用户真实截图：
```
fixtures/
├── qr/
│   ├── v1_mask3.png
│   ├── v10_mask5.png
│   └── ...
├── wx/
│   ├── 36line.png
│   ├── 72line.png
│   └── ...
└── dy/
    ├── 5ring_72pt.png
    ├── 7ring_120pt.png
    └── ...
```

每张图测：识别正确、生成 SVG、与预期 ground-truth 矢量比对（路径数量、模块位置）。

### 8.3 视觉回归
对每张 fixture 生成 SVG 并光栅化为 PNG，与 `tests/golden/` 下的基准 PNG 做像素 diff（差异 > 0.1% 则失败）。

---

## 9. 构建与分发

### 9.1 开发构建
```bash
cargo build           # debug
cargo run             # 启动 GUI
cargo test            # 测试
cargo clippy          # lint
cargo fmt             # 格式化
```

### 9.2 发布构建
```bash
cargo build --release
# 产物：target/release/qracer.exe（预期 ~15MB）
```

### 9.3 优化体积（阶段 6 末）
```toml
[profile.release]
opt-level = "z"        # 优化体积
lto = true             # 链接时优化
codegen-units = 1
strip = true           # 剥离符号
panic = "abort"        # 不要展开
```
预期可压到 ~8-10MB。如要更小，UPX 压缩。

---

## 10. 决策日志

| 日期 | 决策 | 备选 | 选择理由 |
|---|---|---|---|
| 2026-05-28 | 用 Rust 而非 Python | Python + PySide6 | 体积（10MB vs 80MB）、启动时间 |
| 2026-05-28 | egui | slint / iced / Tauri | 纯 Rust 单 crate；immediate-mode 契合工具型 UI |
| 2026-05-28 | 私有码不逆向 | 逆向解码 + 重生成 | 协议未公开，维护成本不可控 |
| 2026-05-28 | EMF 剪贴板（经典） | EMF+ / SVG MIME | AI 兼容性最稳 |
| 2026-05-28 | 不依赖 OpenCV | `opencv` crate | 保持单二进制分发 |
| 2026-05-29 | Windows 前台按键轮询补齐图片粘贴快捷键 | 仅依赖 egui `Key::V`/`Paste` 事件 | egui-winit 对图片剪贴板 `Ctrl+V` 会吞掉按键且不产生文本粘贴事件 |

未来重大变更应追加在此表。

---

## 11. 路线图（高层）

| 阶段 | 范围 | 状态 |
|---|---|---|
| 1 | GUI 骨架、粘贴/打开、左右对比占位 | ✅ 完成 |
| 2 | 预处理 + QR finder 检测 + 透视校正 | ✅ 完成 |
| 3 | QR 主路线：解码 + 8 掩膜重生成 + 差异高亮 | 待 |
| 4 | QR 网格兜底 | 待 |
| 5 | 小程序码识别与采样 | 待 |
| 6 | 抖音码 + EMF 剪贴板 + 打包优化 | 待 |

详细任务分解见 [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md)。
