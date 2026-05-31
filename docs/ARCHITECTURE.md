# QRacer 架构文档

> 文档版本：v1.0  
> 最后更新：2026-05-31  
> 项目状态：阶段 5（小程序码识别与采样）已完成

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
| 粘贴/截屏 | `windows-sys` 0.60 | Windows 前台 `Ctrl+V`/`Shift+Insert` 物理按键状态检测；全屏透明遮罩框选和 GDI 屏幕捕获 |
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
│       │ 粘贴/打开/截屏                                            │
│       ▼                                                           │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │  image_io / screen_capture: 剪贴板、文件、框选截屏         │    │
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
├── screen_capture.rs         Windows 全屏透明遮罩框选 + GDI BitBlt 截屏
│
├── ui/                       UI 层（egui Widget 描述）
│   ├── mod.rs
│   ├── toolbar.rs            顶部工具栏
│   ├── compare_view.rs       左右对比 + 差异高亮
│   └── mask_panel.rs         (阶段 3) 8 掩膜单选 + 网格像素匹配按钮
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
│   └── dy_grid.rs            (阶段 6) 抖音码：同心圆采样
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
└── job/                      (阶段 7 后续可选) 后台任务运行器
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
    pub mask_choice: MaskChoice,           // 当前选用的掩膜 0-7 或网格像素匹配
    pub last_decoded: Option<QrDecoded>,   // 最近一次 QR 解码结果
    pub qr_version: Option<u8>,            // 最近一次推断的 QR 版本，解码失败时仍可用于网格像素匹配
    pub last_matrix: Option<QrMatrix>,     // 最近一次重生成模块矩阵
    pub matched_mask: Option<u8>,          // 中心 Logo 区外完全匹配的 QR 掩膜
    pub last_wx_grid: Option<WxGrid>,      // 最近一次小程序码径向采样结果
    pub last_svg: Option<String>,          // 最近生成的 SVG 文本
    pub last_diff_count: Option<u32>,      // QR 为模块数，小程序码为像素数
    pub show_diff_overlay: bool,           // 是否在右侧预览显示红/蓝差异

    // 后台导入处理：
    processing_job: Option<ProcessingJob>, // 图片识别/校正/采样后台任务
    capture_job: Option<CaptureJob>,       // 截屏框选后台任务
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
5. 从候选中筛选模块尺寸接近、近似右角、面积较大、finder 间 timing pattern 黑白交替合理，且 7×7 finder 模板评分通过的三点组合，作为 QR 三角
```

### 5.2 透视校正
**目标**：把任意角度截图的码摆正到正方形坐标系。

```
1. 三个 finder 中心 + 推算第四个角（向量加法）
2. V2+ QR 优先搜索右下 alignment pattern；命中时用 TL/TR/BL finder 中心和 alignment 中心求模块坐标到原图坐标的单应矩阵，再反推 QR 外框四角；V1 或未命中时回退三 finder 仿射估计
3. 构造源四角 → 目标 (0,0)~(N,N) 的单应矩阵 H（nalgebra）
4. 对原彩色图做反向映射 + 双线性插值 warp 到目标图，再重新二值化；减少拍照图先二值化再拉伸造成的边缘断裂
```

### 5.3 QR 网格采样兜底
**目标**：不解码，直接在校正后图上采样每个模块。

```
1. 从校正图 finder/timing/format 区评分推断 QR 版本 → size = (version-1)*4 + 21
2. 每模块边长 = 目标图边长 / size
3. 对每个 (i, j)：
   - 取 3x3 中心子区域，多数投票（>=5 黑即判黑）
   - 写入 modules[i*size + j]
4. 可解码 QR 会用原始 mask 的解码重生成矩阵作为稳定参考；若直接采样矩阵与参考差异不超过 10% 模块，则输出参考矩阵，避免中心 Logo、屏幕纹理和拍照透视导致同一码输出不一致
5. 输出：SVG 由 size*size 个 rect 拼成；右侧预览与校正图比较差异
6. UI 中解码失败但版本可推断时，仍允许点击"网格像素匹配"，此时保留纯网格采样结果
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

阶段 5 当前实现采用几何启发式：先用黑色连通域和同心嵌套关系找三牛眼，必要时用标准左上/右上/左下象限模板匹配兜底；模板兜底会保留每个角区域的多个候选，避免灰度图中单个假候选挤掉真实牛眼。三点选择会同时检查近似等腰直角三角形、“牛眼外径 / 三角边长”比例，以及右下徽标几何位置；检测到徽标时，会优先选择推算右下角接近徽标的三牛眼组合。小程序码会先用原图三牛眼和右下徽标作为锚点校正到标准正向画布，再用目标画布上的标准三牛眼几何采样，避免旋转/轻微透视/扭曲直接进入 SVG。校正阶段先 warp 原彩色图，再对校正图重新二值化，减少二值图先 warp 带来的边缘断裂。

右下徽标检测不依赖绿色：它提取非白、非黑的大圆形/椭圆形连通域，并使用包围盒中心作为锚点，支持绿色、灰度或其它颜色的固定徽标。照片样本中浅灰背景和黑色码点会干扰连通域，因此徽标像素会同时排除过暗黑码点和浅色背景；连通域失败时，会在右下区域做圆盘形状模板扫描兜底。中心 Logo 不参与校正，因为 Logo 可能是任意形状和颜色。选中三牛眼后按近似正交轴归一化，消除单个牛眼 1-2px 的检测偏移导致的 SVG 共线误差；再通过角向相位搜索和重建误差推断 36/54/72 线版本。SVG 与预览均由同一份 `WxGrid` 径向采样结果生成，并按小程序码实际形态绘制填充圆角矩形/圆点、三牛眼定位点和右下小程序徽标。校正预览当前以 1024px 光栅化显示，右下徽标的预览 S 使用平滑曲线绘制，导出 SVG 则使用标准 SVG 子路径缩放。黑色码点粗细参考 `标准小程序码.svg`，约等于一个径向采样步长。三牛眼和徽标覆盖区会作为保留区跳过采样，避免生成原图没有的码点。

`samples/` 下 9 张标准小程序码和根目录拍照样本已纳入回归，其中 `小程序码9.png` 断言为 72 线，并限制标准样本像素差异上限；根目录 `拍照1.jpg`、`拍照2.jpg`、`拍照3.jpg` 会先校正再采样，并与 `标准.jpg` 的采样矩阵逐位对比，当前要求差异 = 0。遮挡和更低清晰度样本仍需继续补齐。

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
1. 阶段 3 已实现模块级对比：按 QR 版本把校正图分成 NxN 模块
2. 每个模块取中心 3x3 多数投票，与 qrcodegen 重生成矩阵比较
3. 原图黑、生成图白的模块染红；原图白、生成图黑的模块染蓝
4. QR 原掩膜显示不是只看 format info；8 种掩膜中必须存在一个在中心 Logo 区外差异为 0 的矩阵才算匹配。中心 Logo 区内的少量差异不影响匹配；只要中心外有差异，就显示"无匹配掩膜"，不显示"原掩膜 x"，并自动切到网格像素匹配采样
5. 小程序码使用 `wx_grid_to_diff_preview_image` 做像素级对比，红/蓝含义与 QR 一致，徽标和中心 Logo 区域忽略
6. 掩膜面板/小程序码面板都提供"显示差异"开关；状态栏在差异数后写明红/蓝含义
7. 原图坐标系半透明叠加需要保留 homography 后再做反投影，可作为后续增强
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

### 7.1 当前后台处理
粘贴、打开或截屏得到 `DynamicImage` 后，UI 线程立即写入 `original` 并清空旧预览，状态栏显示 spinner + 进度条。预处理、码类型识别、透视校正、解码/采样、SVG 和预览生成放到后台线程执行，通过 `std::sync::mpsc` 把 `ProcessResult` 发回 UI。

UI 线程每帧 `try_recv()` 拉取结果，不阻塞；后台任务未完成时持续 `request_repaint_after()`，保证 loading 状态刷新。用户再次导入图片时会替换当前 receiver，旧任务结果自然丢弃。

### 7.2 截屏框选
点击"截屏"后先向主窗口发送 `ViewportCommand::Minimized(true)`，后台线程短暂等待窗口最小化，再创建 Win32 顶层透明遮罩窗口。用户拖拽框选区域后，遮罩销毁，使用 GDI `BitBlt` 捕获所选屏幕矩形，并按普通导入流程进入后台处理；Esc 或右键取消时恢复主窗口并写入状态栏。截屏线程结束时会主动 `request_repaint()`，排队 `ViewportCommand::Minimized(false)`，并用 Win32 `FindWindowW("QRacer")` + `ShowWindow(SW_RESTORE)` 做原生还原兜底，确保任务栏入口保留且窗口可恢复。

### 7.3 后续抽象
当前只有图片导入处理和截屏两个后台任务，逻辑直接收敛在 `QRacerApp` 内。若阶段 6/7 引入 EMF 写剪贴板、批量处理或更长耗时任务，再抽出独立 `job::runner`。

---

## 8. 测试策略

### 8.1 单元测试
- `detect/finder_qr.rs`：用合成 QR 图（`qrcodegen` 生成）验证 finder 检测
- `detect/finder_wx.rs`：用合成三牛眼验证小程序码定位点检测和三点选择
- `pipeline/preprocess.rs`：Otsu 阈值在已知样本上的稳定性
- `codec/qr.rs`：解码再生成的双向闭环、format info 掩膜读取、版本推断
- `codec/qr_grid.rs`：按校正图采样恢复 QR 模块矩阵、版本推断兜底
- 根目录标准/拍照 QR：验证 `标准.jpg`、`拍照1.jpg`、`拍照2.jpg`、`拍照3.jpg` 先校正再采样；解码重生成矩阵和网格像素匹配输出都必须与标准视图完全一致
- `codec/wx_grid.rs`：按极坐标采样恢复合成径向小程序码网格
- `samples/` 标准小程序码：验证小程序码端到端识别、三牛眼选择、版本推断、采样输出和像素差异上限
- 根目录标准/拍照小程序码：验证非圆形中心 Logo、照片透视、浅灰背景和屏幕纹理样本先校正再采样，并与 `标准.jpg` 的采样矩阵完全一致
- `vector/{svg,diff,shapes}.rs`：SVG rect/扇形输出、模块差异统计、红/蓝分类差异预览

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
| 2026-05-29 | 阶段 3 先同步执行，不引入后台任务器 | 立即引入 `job::runner` + mpsc | 当前 QR 解码和 8 掩膜对比耗时低，阶段 7 按卡顿反馈再加线程模型 |
| 2026-05-29 | 从校正图 format info 自读 QR 原始掩膜 | 依赖 rxing 元数据 | rxing 不稳定暴露 mask；QR format 区坐标固定，自读可控 |
| 2026-05-29 | QR 网格像素匹配只依赖校正图 + 版本推断 | 必须先解码 QR payload | 可处理 payload 解码失败或重生成不可靠的截图，优先保证几何 1:1 |
| 2026-05-31 | 可解码 QR 的网格像素匹配用解码矩阵做稳定参考 | 完全信任拍照图直接采样 | 同一码的拍照样本会受中心 Logo、屏幕纹理和透视残差影响；在直接采样与参考差异较小时用解码矩阵可保证标准视图一致 |
| 2026-05-31 | QR 拍照校正先 warp 原彩色图再重新二值化 | 先二值化再 warp | 彩色图保留抗锯齿边缘和灰度信息，重二值化后 finder、timing 和网格采样更稳定 |
| 2026-05-31 | 导入后立即显示原图，识别/矢量化后台执行 | 导入后同步跑完整管线 | 拍照样本和小程序码采样耗时会上升，同步处理会让用户误以为软件卡死；loading 状态能明确反馈进度 |
| 2026-05-31 | 截屏框选用 Win32 透明遮罩 + GDI 捕获 | 依赖系统截图工具或手动粘贴 | 印前工作流常从屏幕局部取码；内置框选可减少手动截图、保存、再导入的步骤 |
| 2026-05-31 | QR 原掩膜显示必须通过中心 Logo 区外完全匹配验证，无匹配时自动网格采样 | 只要 format info 读到 mask 就显示并重生成 | 部分 QR 的重生成矩阵与原图 8 种掩膜都不匹配；中心 Logo 造成的差异可忽略，但中心外差异说明无匹配掩膜，应直接按校正图采样 |
| 2026-05-31 | 小程序码非标准图先校正彩色图再采样 | 直接在原二值图上采样 | 彩色图校正后重新二值化能保留徽标和抗锯齿边缘，旋转/透视/扭曲样本更稳定 |
| 2026-05-31 | 右下徽标按形状检测，中心 Logo 不作为锚点 | 依赖绿色阈值和中心 Logo | 徽标可能是灰度或非绿色，中心 Logo 形状不可控；固定徽标形状和径向网格规律更稳定 |
| 2026-05-31 | 拍照样本徽标像素排除黑码点和浅灰背景 | 只用暗像素阈值 | 屏幕拍照背景会低于纯白，暗像素阈值会把背景和码点并入徽标候选，影响第四锚点和保留区 |

未来重大变更应追加在此表。

---

## 11. 路线图（高层）

| 阶段 | 范围 | 状态 |
|---|---|---|
| 1 | GUI 骨架、粘贴/打开、左右对比占位 | ✅ 完成 |
| 2 | 预处理 + QR finder 检测 + 透视校正 | ✅ 完成 |
| 3 | QR 主路线：解码 + 8 掩膜重生成 + 差异高亮 | ✅ 完成 |
| 4 | QR 网格像素匹配 | ✅ 完成 |
| 5 | 小程序码识别与采样 | ✅ 完成 |
| 6 | 抖音码 + EMF 剪贴板 + 打包优化 | 待 |

详细任务分解见 [IMPLEMENTATION_PLAN.md](./IMPLEMENTATION_PLAN.md)。
