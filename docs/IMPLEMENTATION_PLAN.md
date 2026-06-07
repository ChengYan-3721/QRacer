# QRacer 分步实施计划

> 文档版本：v1.0  
> 最后更新：2026-06-05
> 适用对象：AI 开发者（自主执行）+ 人类审阅者

本文档把 [ARCHITECTURE.md](./ARCHITECTURE.md) 的路线图拆解为可单独 PR / 单独执行的任务。**每个任务包含：依赖、目标、详细步骤、文件清单、验收标准**。AI 开发者可以按顺序读、直接照做。

---

## 执行约定

1. **顺序执行**：除非任务标注 "可并行"，按编号顺序做
2. **完成后验证**：每个任务完成后必须运行 `cargo build` + `cargo clippy --no-deps -- -D warnings` 通过
3. **测试驱动**：所有新增算法必须配 `#[cfg(test)]` 单元测试
4. **不破坏已有功能**：每个阶段结束时，前面阶段的功能都仍可工作
5. **提交前自检**：参考每个任务末尾的"验收清单"
6. **样本依赖**：阶段 2 起需要 `assets/samples/` 下的真实截图。如缺，先用 `qrcodegen` 合成 QR 样本顶上
7. **不引入 OpenCV**：保持单二进制分发目标，所有图像算法用 `image` / `imageproc` / 手写

---

## 阶段 1：GUI 骨架（已完成）

**完成内容**：
- Cargo 项目初始化（edition 2024）
- 依赖：eframe / egui / egui_extras / image / arboard / windows-sys / rfd / anyhow
- `src/main.rs`：eframe 启动
- `src/app.rs`：`QRacerApp` 状态 + immediate-mode `update()`
- `src/code_kind.rs`：`CodeKind` 枚举
- `src/image_io.rs`：剪贴板读图、文件对话框、纹理转换
- `src/screen_capture.rs`：Windows 全屏透明遮罩框选 + GDI 截屏导入
- `src/ui/{toolbar, compare_view}.rs`：工具栏 + 左右对比
- 导入体验优化（2026-05-31）：粘贴、打开或截屏得到图片后先显示原图；预处理、识别、校正、解码/采样、SVG 和预览生成在后台线程执行，状态栏和预览区显示 loading
- 工具栏新增"截屏"按钮：点击后隐藏本应用窗口，显示全屏透明遮罩；拖拽框选后自动把截图区域导入并开始后台处理，Esc/右键取消

**验收已通过**：`cargo build` 无 warning；窗口启动正常；粘贴/打开能加载图像；导入后原图先显示且处理期间有 loading 状态。

---

## 阶段 2：预处理 + QR 定位 + 透视校正（已完成）

**目标**：把任意截图中的 QR 摆正，输出一张固定尺寸的二值校正图，供阶段 3/4 采样。

**完成记录（2026-05-28）**：
- 已新增 `imageproc`、`nalgebra`、`thiserror`，并为阶段 2 单测新增 `qrcodegen` dev-dependency
- 已新增 `src/error.rs`、`src/pipeline/{mod,preprocess,perspective,grid}.rs`、`src/detect/{mod,finder_qr,finder_wx,finder_dy}.rs`
- 已实现 `BinaryImage`、Otsu 二值化、3x3 黑色前景开闭运算、QR finder RLE 检测、`detect_kind` QR 分支、4 点单应矩阵和 512×512 二值透视校正预览
- 已修正 finder 选点策略：不再按 `module` 最大直接取前三个候选，而是按模块尺寸一致性、右角几何和三角面积选择真正的三个 QR 角点，避免数据区误检导致透视校正歪斜
- 已修正旋转场景：三点选择加入 finder 间 timing pattern 黑白交替评分，并收紧近似直角误差阈值，避免顺/逆时针旋转二维码中数据区误检抢占真实 finder
- 已修正轻微透视场景：V2+ QR 优先搜索右下 alignment pattern 作为第 4 个透视约束点；未检测到 alignment pattern 时再回退三 finder 仿射估计
- QR 拍照样本优化（2026-05-31）：三点选择增加 7×7 finder 模板评分，避免拍照图中数据区伪 finder 抢占真实角点；V2+ 透视校正改为用三个 finder 中心 + 右下 alignment 中心直接求单应矩阵，再反推外框四角
- 小红书缩放截图优化（2026-06-07）：三点选择在面积评分之外优先采用可信 QR lattice，避免放大图中圆点数据区形成小尺度伪 finder 后被误选；`小红书QR码1/2` 原图、放大图、缩小图回归要求数据区一致且 SVG 一致
- QR 校正也改为先 warp 原彩色图，再对校正图重新二值化，减少拍照图先二值化再拉伸产生的锯齿和边缘断裂
- 已接入 `QRacerApp::set_original()`：粘贴/打开图片后自动运行预处理、识别、finder 检测和透视校正；非 QR 图片右侧预览为空
- 已修复 UI 中文显示：启动时加载 Windows 系统 CJK 字体注册到 egui，避免默认字体缺字导致中文乱码
- 已修复图片剪贴板快捷键：egui-winit 在图片剪贴板场景会吞掉 `Ctrl+V` 的 `Key::V` 事件且不产生文本 `Paste` 事件，现额外用 Windows 前台按键状态做边沿触发，应用在前台时可直接 `Ctrl+V` 粘贴图片
- 已修复阶段 1 遗留 clippy 问题：`compare_view.rs` 中多余的 `&*tex` 重借用

**验证已通过**：
- `cargo fmt --check`
- `cargo test`（12 个单元测试通过）
- `cargo build`
- `cargo clippy --no-deps -- -D warnings`

**仍需人工/样本验收**：
- 根目录 `标准.jpg`、`拍照1.jpg`、`拍照2.jpg`、`拍照3.jpg` 已纳入 QR 拍照回归；同一组样本的解码重生成和网格像素匹配输出需与标准图一致
- 小程序码/抖音码截图的 UI 手测需要按码类型继续补样本执行

### 任务 2.1：新增依赖 + 公共错误类型

**依赖**：阶段 1 完成

**步骤**：
1. `Cargo.toml` 添加：
   ```toml
   imageproc = "0.25"
   nalgebra = "0.33"
   thiserror = "2"
   ```
2. 新建 `src/error.rs`：
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
       #[error("透视校正失败：{0}")]
       Perspective(String),
       #[error("IO 错误：{0}")]
       Io(#[from] std::io::Error),
       #[error("图像格式错误：{0}")]
       ImageFormat(String),
   }
   
   pub type Result<T> = std::result::Result<T, QRacerError>;
   ```
3. 在 `src/main.rs` 顶部加 `mod error;`

**文件**：`Cargo.toml`、`src/error.rs`、`src/main.rs`

**验收**：`cargo build` 通过；`QRacerError` 可在其他模块用 `use crate::error::{QRacerError, Result};`

---

### 任务 2.2：预处理模块（灰度 + Otsu + 形态学）

**依赖**：2.1

**目标**：把 `DynamicImage` 转成可靠的二值图。

**步骤**：
1. 新建 `src/pipeline/mod.rs`：
   ```rust
   pub mod preprocess;
   pub mod perspective;
   pub mod grid;
   ```
2. 新建 `src/pipeline/preprocess.rs`，定义：
   ```rust
   pub struct BinaryImage {
       pub w: u32,
       pub h: u32,
       pub data: Vec<u8>, // 0 或 255，存 u8 兼容 imageproc::GrayImage
   }
   
   /// 主入口：灰度 → Otsu 阈值 → 形态学开运算去噪
   pub fn preprocess(img: &DynamicImage) -> BinaryImage;
   
   /// 仅做 Otsu 二值化（不去噪）
   pub fn otsu_binarize(gray: &image::GrayImage) -> image::GrayImage;
   ```
3. 实现 Otsu：用 `imageproc::contrast::threshold` 或 `imageproc::contrast::otsu_level` 取阈值后手动 threshold
4. 形态学：用 `imageproc::morphology::{open_mut, close_mut}`，结构元 3×3
5. 在 `src/main.rs` 加 `mod pipeline;`

**单元测试**（`src/pipeline/preprocess.rs` 底部）：
```rust
#[test]
fn otsu_separates_pure_bimodal() {
    // 合成一张 50% 黑、50% 白的图，验证 Otsu 阈值 ≈ 128
}

#[test]
fn preprocess_keeps_qr_modules() {
    // 用 qrcodegen 合成一张干净 QR，验证 BinaryImage 黑模块数量与原图一致
    // （需要先在 dev-dependencies 加 qrcodegen）
}
```

**文件**：`src/pipeline/mod.rs`、`src/pipeline/preprocess.rs`、`src/main.rs`

**验收**：
- [ ] `cargo build` 通过，0 warning
- [ ] `cargo test pipeline::preprocess` 通过
- [ ] 在 GUI 中加临时按钮"显示二值图"，把 BinaryImage 转回 ColorImage 显示在 preview 侧验证

---

### 任务 2.3：QR Finder Pattern 检测

**依赖**：2.2

**目标**：在二值图中找到 QR 的三个 1:1:3:1:1 嵌套方块。

**步骤**：
1. 新建 `src/detect/mod.rs`：
   ```rust
   pub mod finder_qr;
   pub mod finder_wx;  // 阶段 5 实现
   pub mod finder_dy;  // 阶段 6 实现
   
   use crate::code_kind::CodeKind;
   use crate::pipeline::preprocess::BinaryImage;
   
   pub fn detect_kind(bin: &BinaryImage) -> CodeKind;
   ```
2. 新建 `src/detect/finder_qr.rs`，定义：
   ```rust
   #[derive(Debug, Clone, Copy)]
   pub struct QrFinder {
       pub cx: f64,
       pub cy: f64,
       pub module: f64,  // 单模块边长（亚像素）
   }
   
   pub fn find_qr_finders(bin: &BinaryImage) -> Vec<QrFinder>;
   ```
3. **算法**（行扫描 RLE 法）：
   1. 对每行 y：
      - 把行数据转为 RLE 段 `(color, len)`
      - 滑窗看连续 5 段是否近似 1:1:3:1:1（容差 ±50%；总长度 = 7 * module，且 module ≥ 1px）
      - 命中则记录段中心 x 和估算 module
   2. 对每个行命中，垂直方向同样验证（在该 x 列上做 RLE）
   3. 行列双轴验证通过的候选 → 一个 finder
   4. 候选去重：同一中心 ±3px 合并
   5. 返回找到的所有 finders
4. 在 `src/main.rs` 加 `mod detect;`

**单元测试**：
```rust
#[test]
fn finds_three_finders_on_synthetic_qr() {
    // qrcodegen 生成 V5 QR → 渲染成 BinaryImage → 应找到 ≥ 3 个 finder
}

#[test]
fn ignores_finders_in_data_area() {
    // 数据区有时也有 1:1:3:1:1 巧合，必须双轴验证才记录
}
```

**文件**：`src/detect/mod.rs`、`src/detect/finder_qr.rs`、`src/main.rs`

**验收**：
- [ ] 用 `qrcodegen` 合成 V1, V5, V10, V20, V40 五张 QR，每张都能找到至少 3 个 finder
- [ ] 单测通过

---

### 任务 2.4：识别码类型（dispatch）

**依赖**：2.3

**目标**：基于 finder 几何特征判定 `CodeKind`。

**步骤**：
1. 实现 `src/detect/mod.rs::detect_kind`：
   ```rust
   pub fn detect_kind(bin: &BinaryImage) -> CodeKind {
       // 1. 试 QR：如 find_qr_finders 返回 >= 3 个候选 → CodeKind::Qr
       // 2. 试小程序码：finder_wx::find（阶段 5 实现，阶段 2 先返回空）
       // 3. 试抖音码：finder_dy::find（阶段 6 同上）
       // 4. 都失败 → CodeKind::Unknown
   }
   ```
2. 阶段 2 先只实现 QR 分支，其他两个返回 `CodeKind::Unknown` 占位

**文件**：`src/detect/mod.rs`、`src/detect/finder_wx.rs`（空壳）、`src/detect/finder_dy.rs`（空壳）

**验收**：
- [ ] 合成 QR 图 → `detect_kind` 返回 `CodeKind::Qr`
- [ ] 一张白纸 → 返回 `Unknown`

---

### 任务 2.5：透视校正

**依赖**：2.3

**目标**：用三个 finder + 推算的第四角，把斜拍/旋转的 QR 摆正成 N×N 像素的正方形。

**步骤**：
1. 新建 `src/pipeline/perspective.rs`：
   ```rust
   use nalgebra::Matrix3;
   use crate::detect::finder_qr::QrFinder;
   use crate::pipeline::preprocess::BinaryImage;
   
   pub fn warp_qr_to_square(
       bin: &BinaryImage,
       finders: &[QrFinder; 3],
       target_size: u32,
   ) -> BinaryImage;
   
   /// 从四对点求单应矩阵（DLT 法）
   pub fn homography_from_4pts(
       src: &[(f64, f64); 4],
       dst: &[(f64, f64); 4],
   ) -> Matrix3<f64>;
   ```
2. **算法**：
   1. 三个 finder 排序：取互相距离最大的两个作 TL-TR / TL-BL 边
   2. 推第四角 BR = TR + BL - TL（向量加法）
   3. 目标四角 = (0,0), (N,0), (0,N), (N,N)
   4. DLT 求 H（用 nalgebra 求 SVD）
   5. 反向映射：对目标每像素，用 H⁻¹ 反推源坐标，双线性插值
3. 三个 finder 的排序：找到一对距离最大、且第三个在它们垂直方向上 → 那对是 TL-TR 或 TL-BL（用叉积判定方向）

**单元测试**：
```rust
#[test]
fn homography_identity_on_unit_square() {
    // src 和 dst 都是 (0,0)(1,0)(0,1)(1,1) → H = I
}

#[test]
fn warp_synthetic_qr_recovers_modules() {
    // 合成 QR → 透视变换扭曲 → warp 回来 → 像素差异 < 1%
}
```

**文件**：`src/pipeline/perspective.rs`

**验收**：
- [ ] 单测通过
- [ ] 在 GUI 加临时按钮"显示校正图"，扭曲样本 QR 应被摆正

---

### 任务 2.6：阶段 2 整合到 UI

**依赖**：2.2-2.5

**目标**：用户粘贴图后，自动跑预处理 + 识别 + 校正，把校正后的二值图显示在 preview 侧。

**步骤**：
1. 在 `QRacerApp` 增加字段：
   ```rust
   pub binary: Option<BinaryImage>,
   pub finders: Option<Vec<QrFinder>>,
   pub warped: Option<BinaryImage>,
   ```
2. 修改 `set_original()`：调用 preprocess → detect_kind → find_qr_finders → warp_qr_to_square
3. 把 `warped` 转 ColorImage 显示在 preview 侧（替换阶段 1 的"复制原图"占位）
4. `code_kind` 字段更新为识别结果

**文件**：`src/app.rs`、`src/ui/compare_view.rs`（保持不变即可）

**验收**：
- [ ] 粘贴一张真实 QR 截图 → 左侧原图 / 右侧校正后摆正的黑白图
- [ ] 顶部码类型显示"二维码 (QR)"
- [ ] 拍歪的 QR 也能摆正
- [ ] 非 QR 图（如风景照）显示"未识别"，preview 侧空

---

## 阶段 3：QR 主路线（解码 + 8 掩膜重生成 + 差异高亮）（已完成）

**完成记录（2026-05-29）**：
- 已新增 `rxing`、`qrcodegen` 为运行时依赖，`qrcodegen` 现在同时服务合成测试和 QR 重生成
- 已新增 `src/codec/{mod,qr}.rs`：使用 `rxing::MultiFormatReader` 解码 QR 文本/ECC；从校正图 format info 读取原始掩膜；通过 finder/timing/format 区评分推断 QR 版本
- 已实现 `regenerate_qr()`：按解码文本、版本、纠错等级和指定 mask 0-7 调用 `qrcodegen::encode_segments_advanced()` 重生成模块矩阵
- 已新增 `src/vector/{mod,svg,diff,shapes}.rs`：输出 QR SVG；按校正图模块中心多数投票计算差异；右侧预览中用红色标记"原图有、生成图没有"，用蓝色标记"原图没有、生成图有"
- 已新增 `src/ui/mask_panel.rs` 并接入 `QRacerApp`：支持 0-7 掩膜单选、自动选择差异最少掩膜、显示/隐藏差异开关、显示版本/ECC/原始掩膜/差异模块数
- 原掩膜显示优化（2026-05-31）：format info 读到的 mask 仅作为候选；8 种掩膜中必须有一个在中心 Logo 区外差异为 0 才显示"原掩膜 x"。中心 Logo 区内差异忽略，中心外存在任意差异则显示"无匹配掩膜"，并自动使用网格像素匹配采样
- 已启用工具栏"导出 SVG"按钮：生成成功后可保存 `.svg` 文件；"复制到剪贴板"已在任务 6.4 改为直接复制当前 SVG 文本到剪贴板
- "网格像素匹配"按钮已接入阶段 4 采样兜底流程

**验证已通过**：
- `cargo fmt --check`
- `cargo test`（21 个单元测试通过）
- `cargo build`
- `cargo clippy --no-deps -- -D warnings`

**仍需人工/样本验收**：
- 当前仍缺少 `assets/samples/` 真实 QR 截图 fixture；阶段 3 自动化验证使用 `qrcodegen` 合成 QR 样本
- 真实截图中的复杂编码模式（如 ECI/Kanji/非 UTF-8 字节段）可能无法由 `qrcodegen::QrSegment::make_segments()` 1:1 复现，后续需要真实样本确认并决定是否扩展段级解码
- 原图坐标系上的半透明差异叠加还未做；当前阶段 3 在右侧矢量预览中用红/蓝颜色区分差异模块，原图反投影叠加可在保留 homography 后扩展

### 任务 3.1：新增依赖

**步骤**：
```toml
rxing = "0.7"
qrcodegen = "1.8"
```

**验收**：`cargo build` 通过

---

### 任务 3.2：QR 解码

**目标**：从原图（或校正图）中解码出文本 + 元数据（版本、纠错等级、原掩膜编号）。

**步骤**：
1. 新建 `src/codec/mod.rs`：
   ```rust
   pub mod qr;
   pub mod qr_grid;
   pub mod wx_grid;
   pub mod dy_grid;
   ```
2. 新建 `src/codec/qr.rs`：
   ```rust
   use rxing::{BarcodeFormat, DecodeHints, MultiFormatReader, Reader};
   
   pub struct QrDecoded {
       pub text: String,
       pub version: u8,        // 1-40
       pub ecc: QrEcc,         // L/M/Q/H
       pub original_mask: Option<u8>, // rxing 不一定能给到，可能需要从 format info 自己解
   }
   
   pub enum QrEcc { L, M, Q, H }
   
   pub fn decode_qr(img: &image::DynamicImage) -> crate::error::Result<QrDecoded>;
   ```
3. 用 `rxing::helpers::detect_in_luma` 或 `MultiFormatReader::decode` 解码
4. 元数据提取：rxing 的 `RXingResult` 带 `result_metadata`，提取 version 和 ECC
5. **原掩膜编号**：rxing 通常不暴露。可以读取校正后图的 format info 区（坐标固定）→ 取出 5 bits format info → XOR `0b101010000010010` 去掩膜 → 前 2 bits 是 ECC，后 3 bits 是 mask。这部分自己写一个 `read_format_info(warped: &BinaryImage) -> (QrEcc, u8)` 函数

**单元测试**：
```rust
#[test]
fn decode_synthetic_qr() {
    let text = "https://example.com";
    let qr = qrcodegen::QrCode::encode_text(text, qrcodegen::QrCodeEcc::Medium).unwrap();
    let img = render_qrcodegen(&qr);
    let decoded = decode_qr(&img).unwrap();
    assert_eq!(decoded.text, text);
    assert_eq!(decoded.version, qr.version().value());
}
```

**文件**：`src/codec/mod.rs`、`src/codec/qr.rs`

**验收**：5 张不同版本的合成 QR 都能正确解码出文本和版本

---

### 任务 3.3：QR 重生成（8 种掩膜可控）

**目标**：给定解码结果 + 指定 mask（0-7），输出一份"标准化"的矢量 QR。

**步骤**：
1. 在 `src/codec/qr.rs` 增加：
   ```rust
   pub fn regenerate_qr(decoded: &QrDecoded, mask: u8) -> crate::error::Result<Vec<Vec<bool>>>;
   ```
2. 实现：
   ```rust
   use qrcodegen::{QrCode, QrCodeEcc, QrSegment, Version, Mask};
   
   let ecc = match decoded.ecc {
       QrEcc::L => QrCodeEcc::Low,
       QrEcc::M => QrCodeEcc::Medium,
       QrEcc::Q => QrCodeEcc::Quartile,
       QrEcc::H => QrCodeEcc::High,
   };
   let v = Version::new(decoded.version);
   let segs = QrSegment::make_segments(&decoded.text);
   let qr = QrCode::encode_segments_advanced(
       &segs, ecc, v, v, Some(Mask::new(mask)), false
   )?;
   let size = qr.size() as usize;
   let mut matrix = vec![vec![false; size]; size];
   for y in 0..size {
       for x in 0..size {
           matrix[y][x] = qr.get_module(x as i32, y as i32);
       }
   }
   Ok(matrix)
   ```

**单元测试**：
```rust
#[test]
fn regenerate_with_mask_3_matches_canonical_mask_3() {
    // 用 qrcodegen 直接生成 mask=3 的 QR，再走 decode→regenerate(mask=3)，模块矩阵应完全一致
}
```

**文件**：`src/codec/qr.rs`

**验收**：单测通过

---

### 任务 3.4：SVG 输出（QR）

**目标**：从模块矩阵 `Vec<Vec<bool>>` 生成 SVG 文本。

**步骤**：
1. 新建 `src/vector/mod.rs`：
   ```rust
   pub mod svg;
   pub mod shapes;
   pub mod diff;
   ```
2. 新建 `src/vector/svg.rs`：
   ```rust
   pub struct SvgBuilder {
       w: f64,
       h: f64,
       body: String,
   }
   
   impl SvgBuilder {
       pub fn new(w: f64, h: f64) -> Self;
       pub fn rect(&mut self, x: f64, y: f64, w: f64, h: f64, fill: &str);
       pub fn circle(&mut self, cx: f64, cy: f64, r: f64, fill: &str);
       pub fn path(&mut self, d: &str, fill: &str);
       pub fn finish(self) -> String; // <svg ...>{body}</svg>
   }
   
   pub fn qr_matrix_to_svg(matrix: &[Vec<bool>], module_mm: f64) -> String;
   ```
3. `qr_matrix_to_svg`：每个 `true` 模块画一个 `<rect>`，边长 = `module_mm`
4. SVG 头：`width / height` 用 mm 单位，`viewBox="0 0 size size"`，黑模块 fill="#000"

**单元测试**：
```rust
#[test]
fn svg_module_count_matches() {
    let m = vec![vec![true, false], vec![false, true]];
    let svg = qr_matrix_to_svg(&m, 1.0);
    assert_eq!(svg.matches("<rect").count(), 2);
}
```

**文件**：`src/vector/mod.rs`、`src/vector/svg.rs`、`src/vector/shapes.rs`（先空）、`src/vector/diff.rs`（任务 3.5 实现）

**验收**：把生成的 SVG 用浏览器打开，能看到正确的 QR

---

### 任务 3.5：差异高亮

**目标**：把生成的 QR 矩阵与原图二值化结果对比，找出差异的模块位置，叠加红色到原图预览。

**步骤**：
1. 新建 `src/vector/diff.rs`：
   ```rust
   pub struct DiffResult {
       pub diff_modules: Vec<(u32, u32)>, // 不一致模块的 (i,j)
       pub overlay: egui::ColorImage,     // 同原图尺寸，差异处红色半透明
   }
   
   pub fn compute_diff(
       warped_original: &BinaryImage,
       generated_matrix: &[Vec<bool>],
       original_size: (u32, u32),  // 原图（未校正）尺寸
       homography_inv: &nalgebra::Matrix3<f64>, // 用于把校正坐标反映射回原图
   ) -> DiffResult;
   ```
2. 算法：
   1. 对校正后图的每个模块格 (i,j)，比较 `warped_original` 的多数投票值 vs `generated_matrix[i][j]`
   2. 不一致 → 用 H⁻¹ 把模块四角映射回原图坐标
   3. 在 overlay 上把那四角围成的多边形涂红（半透明）
3. 返回 `DiffResult`，UI 把 overlay 叠加在原图侧

**文件**：`src/vector/diff.rs`

**验收**：把同一文本用 mask=3 和 mask=5 各生成一次，差异高亮应该能清楚标出哪些模块不同

---

### 任务 3.6：掩膜面板 + 自动 8 掩膜对比

**目标**：UI 上添加 0-7 掩膜单选 + "网格像素匹配"按钮，以及"自动尝试所有掩膜"按钮。

**步骤**：
1. 新建 `src/ui/mask_panel.rs`：
   ```rust
   pub fn show(ui: &mut egui::Ui, app: &mut QRacerApp);
   ```
   - 8 个掩膜单选（RadioButton）
   - "网格像素匹配"按钮（阶段 4 接入）
   - "自动选最佳掩膜"按钮（自动跑 8 种，取差异最少的）
   - 显示当前差异模块数
2. 在 `QRacerApp` 增加：
   ```rust
   pub mask_choice: MaskChoice,
   pub last_decoded: Option<QrDecoded>,
   pub last_diff_count: Option<u32>,
   
   pub enum MaskChoice {
       Mask(u8),
       GridFallback,
   }
   ```
3. 当 `mask_choice` 改变 → 调用 `regenerate_qr` + `qr_matrix_to_svg` + `compute_diff`，刷新 preview 和 overlay
4. "自动选最佳"：8 次循环，记差异最少的 mask，把 `mask_choice` 设为它

**文件**：`src/ui/mask_panel.rs`、`src/ui/mod.rs`、`src/app.rs`

**验收**：
- [ ] 切换掩膜，preview 侧 SVG 立即更新
- [ ] "自动选最佳"能找到与原图一致的掩膜
- [ ] 差异模块数显示正确

---

### 任务 3.7：导出 SVG 文件

**步骤**：
1. `src/app.rs` 增加：
   ```rust
   pub fn try_export_svg(&mut self) {
       let Some(svg) = &self.last_svg else { return; };
       if let Some(path) = rfd::FileDialog::new()
           .add_filter("SVG", &["svg"])
           .save_file()
       {
           std::fs::write(&path, svg).ok();
       }
   }
   ```
2. `toolbar.rs` 启用"导出 SVG"按钮（不再禁用）

**验收**：点导出能保存 .svg 文件，AI 打开正常

---

## 阶段 4：QR 网格像素匹配（已完成）

**完成记录（2026-05-29）**：
- 已新增 `src/codec/qr_grid.rs`：通过阶段 3 的 QR 版本推断能力识别模块数，并在校正图上按模块中心 3×3 多数投票采样生成 `QrMatrix`
- 已接通 `MaskChoice::GridFallback`：点击"网格像素匹配"后不再走 `qrcodegen` 重生成，而是直接采样校正图生成 SVG 和右侧预览
- 已支持"解码失败但版本可推断"场景：阶段 2 校正成功后，即使 QR payload/ECC 元数据解码失败，也会自动使用"网格像素匹配"生成矩阵、SVG 和右侧预览；版本推断失败时再用三 finder 距离估算模块数
- 无匹配掩膜拍照样本优化（2026-05-31）：网格采样前会围绕整体 shift/scale 做小范围自校准，用 finder、separator、timing、alignment pattern 和全模块采样置信度评分；3×3 采样投票改为 9 点中 >=4 黑即判黑，减少拍照模糊对边缘黑点的侵蚀；该路径直接输出采样矩阵，不依赖解码重生成矩阵兜底
- QR 拍照样本优化（2026-05-31）：当 QR 已能解码时，网格像素匹配会用原始 mask 的解码重生成矩阵作为稳定参考；若直接采样矩阵与参考差异在 10% 模块以内，则输出参考矩阵，避免中心 Logo、屏幕纹理和拍照透视造成同一码不同图；解码失败时仍保留纯网格像素匹配
- QR 差异预览修正（2026-05-31）：生成阶段保存 `sample_qr_grid` 的参考矩阵，预览差异和掩膜候选比较改为参考矩阵与生成矩阵逐模块比较，不再从校正图用旧采样器二次采样，避免 SVG 正确但预览出现假高亮
- 根目录 `标准.jpg`、`拍照1.jpg`、`拍照2.jpg`、`拍照3.jpg` 已纳入 QR 回归：解码再重建矩阵逐位一致，网格像素匹配回归直接比较 `sample_qr_grid` 原始采样矩阵，不再用重生成矩阵吸附；3 张拍照图与标准图逐位一致
- 掩膜单选和"自动选最佳"在未解码时禁用；"网格像素匹配"只依赖校正图和版本推断；当已解码 QR 无匹配掩膜时也会自动走网格像素匹配，且不再用解码重生成矩阵替换采样矩阵

**验证已通过**：
- `cargo fmt --check`
- `cargo test`（23 个单元测试通过）
- `cargo build`
- `cargo clippy --no-deps -- -D warnings`

**仍需人工/样本验收**：
- 当前仍缺少真实损坏 QR 截图 fixture；自动化验证已覆盖合成无噪 QR 和根目录拍照 QR，但无法解码的真实损坏 QR 仍需补样本
- 标准 `cargo build` 若被正在运行的 `target\debug\qracer.exe` 占用，会在最终验证时改用独立 target 目录确认构建

### 任务 4.1：网格采样实现

**目标**：不解码，直接在校正图上按 QR 版本对应的网格采样，输出模块矩阵。

**步骤**：
1. 新建 `src/codec/qr_grid.rs`：
   ```rust
   pub fn sample_qr_grid(
       warped: &BinaryImage,
       version: u8,
   ) -> Vec<Vec<bool>>;
   ```
2. 算法：
   - `size = (version-1)*4 + 21`
   - 每模块边长 = `warped.w / size`
   - 对每个 (i, j)：取中心 3×3 子区域，多数投票
3. **版本检测兜底**：如果用户没指定版本，从 finder 的 `module` 字段推算（warped.w / module ≈ size）

**单元测试**：
```rust
#[test]
fn grid_sampling_recovers_perfect_qr() {
    // 合成无噪 QR → 校正后图 → 网格采样 → 应与 qrcodegen 矩阵 100% 一致
}
```

**文件**：`src/codec/qr_grid.rs`

**验收**：单测通过

---

### 任务 4.2：UI 接通网格像素匹配

**步骤**：
1. `MaskChoice::GridFallback` 分支：调用 `sample_qr_grid` 而非 `regenerate_qr`
2. 显示状态："使用网格像素匹配（保证 1:1 还原）"

**验收**：
- [ ] 点"网格像素匹配"按钮，preview 侧应与原图模块完全一致（差异 = 0）
- [ ] 用一张"无解码可能"的损坏 QR（手动把数据区涂掉几个模块），网格像素匹配仍能输出原样矢量

---

## 阶段 5：小程序码识别与采样（已完成）

**完成记录（2026-05-29）**：
- 已实现 `src/detect/finder_wx.rs`：通过黑色连通域、圆度近似和同心嵌套关系检测小程序码三牛眼定位点，并在 `detect_kind()` 中接入 `CodeKind::WxMiniprogram`
- 已新增 `src/codec/wx_grid.rs`：根据三牛眼推算极坐标几何，按 36/54/72 线版本进行径向采样，每线固定 13 点
- 已新增 `vector::shapes::polar_sector_path()`，并在 `src/vector/svg.rs` 中支持小程序码圆角矩形/圆点 SVG 输出和右侧预览光栅化
- 已接入 `QRacerApp`：粘贴/打开小程序码后自动识别、采样、生成 SVG；小程序码掩膜面板隐藏，显示“重新采样”和“显示差异”入口，工具栏“导出 SVG”复用既有流程

**调参记录（2026-05-30）**：
- 已用 `samples/` 下 9 张标准小程序码调参：三牛眼检测增加标准左上/右上/左下象限模板匹配兜底，并按等腰直角三角形约束选择定位点
- 径向采样内半径改为从中心 Logo 外侧开始，外半径按标准样本调到 `牛眼中心半径 + 1.41 * 牛眼外半径`；角向采样增加相位搜索，36/54/72 推断改为优先选择重建误差最低的候选
- SVG/预览从扇形块改为小程序码实际形态的填充圆角矩形/圆点，并补绘三牛眼定位点
- 采样时把三牛眼和右下小程序徽标覆盖区作为保留区，不再生成原图没有的黑色码点；右下徽标从原图按形状检测并矢量绘制，颜色可为绿色或灰度
- 已参考 `标准小程序码.svg` 校准黑色码点粗细：圆角矩形宽度约等于一个径向采样步长；右下徽标的白色 S 使用标准 SVG 的 S 子路径缩放生成
- 已增加小程序码像素级差异预览：红色表示原图有、生成图没有，蓝色表示原图没有、生成图有；UI 中可用“显示差异”开关控制
- `samples/` 回归增加 `小程序码9.png` 识别为 72 线断言，并限制标准样本生成预览的像素差异上限
- 已修正 `小程序码9.png` 三牛眼微小检测偏差：选中三牛眼后按近似正交轴归一化，让右上/左下牛眼与左上牛眼严格共线，同时保留实际横向/纵向距离
- 校正预览光栅尺寸从 512 提升到 1024；左右对比图在面板剩余空间内居中显示；右下小程序徽标的 S 预览改为平滑曲线绘制，导出 SVG 仍使用标准路径
- 已接入小程序码校正前置步骤：先用原图三牛眼和右下徽标作为锚点把旋转/轻微透视/扭曲图拉正到标准正向画布，再在校正图上采样、预览和导出 SVG；中心 Logo 不参与校正，因为 Logo 形状和颜色不可作为稳定标准
- 非标准样本优化（2026-05-31）：三牛眼选点增加“牛眼外径 / 三角边长”比例约束，避免扭曲图中大码点连通域被误选为定位点；小程序码校正改为先 warp 原彩色图，再对校正图重新二值化采样
- 右下徽标检测从绿色阈值改为形状检测：提取非白、非黑的大圆形/椭圆形连通域，支持标准灰度图和非绿色徽标；照片样本会额外排除黑色码点和浅灰背景，连通域失败时在右下区域做圆盘模板扫描兜底；三牛眼选择在检测到徽标时增加“推算右下角接近徽标”的几何评分，用径向网格的标准分布判断修复方向
- 根目录 `标准.jpg`、`拍照1.jpg`、`拍照2.jpg`、`拍照3.jpg` 已纳入回归：拍照图先校正再采样，并与 `标准.jpg` 的采样矩阵逐位对比，当前要求差异 = 0

**验证已通过**：
- `cargo fmt --check`
- `cargo test`（32 个单元测试通过，包含 `samples/` 标准小程序码和根目录 6 张标准/灰度/变换样本回归）
- `cargo build`
- `cargo clippy --no-deps -- -D warnings`

**仍需人工/样本验收**：
- 当前已覆盖标准正向、非圆形中心 Logo、真实拍照透视、浅灰背景和屏幕纹理样本；遮挡、严重模糊和更低清晰度截图仍需继续补样本验证
- 小程序码 payload 属于私有编码，本阶段只做几何识别、采样和矢量化，不做逆向解码
- 当前小程序码差异高亮为右侧预览的像素级 overlay；原图坐标系的半透明反投影叠加仍可作为后续增强

### 任务 5.1：小程序码 Finder（三牛眼）

**目标**：找三个圆形嵌套的牛眼定位点。

**步骤**：
1. 实现 `src/detect/finder_wx.rs`：
   ```rust
   pub struct WxFinder {
       pub cx: f64,
       pub cy: f64,
       pub r_outer: f64,  // 外圆半径
   }
   
   pub fn find_wx_finders(bin: &BinaryImage) -> Vec<WxFinder>;
   ```
2. 算法（**纯 Rust，不依赖 Hough Circle**）：
   1. `imageproc::contours::find_contours` 提取所有黑色轮廓
   2. 对每个轮廓计算：
      - 圆度 = 4π·Area / Perimeter²，> 0.85 即近圆
      - 外接圆中心 + 半径
   3. 找嵌套对：两个圆度高的轮廓，圆心距 < 较小半径 → 嵌套关系
   4. 牛眼 = 至少 2 层嵌套圆
   5. 三个最大的牛眼即为定位点

**文件**：`src/detect/finder_wx.rs`

**验收**：用真实小程序码截图测试，能找到 3 个牛眼

---

### 任务 5.2：小程序码元信息解析

**目标**：解出版本（36/54/72 线）。

**步骤**：
1. 在 `src/codec/wx_grid.rs` 中：
   ```rust
   pub fn detect_wx_version(
       bin: &BinaryImage,
       finders: &[WxFinder; 3],
   ) -> crate::error::Result<u32>; // 返回 36/54/72
   ```
2. 算法：
   - 三个牛眼确定圆心和外径 R
   - 在第 4 个点位置（统一坐标）采样元信息（位置在所有版本一致）
   - 元信息位（3 bits？）编码版本枚举
   - **注**：具体元信息编码可能需要逆向真实样本来确定。先实现"通过整体径向密度判定"作为备选：径向线条数最容易直接数

**备选算法**：直接数径向线数
- 圆心到外圈做 360° 等步采样
- 统计黑白交替次数 = 线数

**文件**：`src/codec/wx_grid.rs`

**验收**：能正确识别 36/54/72 三个版本

---

### 任务 5.3：小程序码采样

**目标**：在径向网格上采样，输出 `samples: Vec<bool>` 和几何参数。

**步骤**：
1. `src/codec/wx_grid.rs`：
   ```rust
   pub struct WxGrid {
       pub center: (f64, f64),
       pub r_min: f64,
       pub r_max: f64,
       pub lines: u32,
       pub points_per_line: u32, // 固定 13
       pub samples: Vec<bool>,
   }
   
   pub fn sample_wx(
       bin: &BinaryImage,
       finders: &[WxFinder; 3],
       version: u32,
   ) -> crate::error::Result<WxGrid>;
   ```
2. 算法：
   - 圆心 = 三牛眼几何中心
   - r_max = 最远牛眼到圆心距离（按小程序码规范，牛眼在最外几条线）
   - r_min = 最内填充区半径（参考规范）
   - 对每个 (line, point) 极坐标采样
   - 注：第 1 点（最内填充）和第 13 点（最外填充）不参与编码，但矢量化要保留这些填充模式

**文件**：`src/codec/wx_grid.rs`

**验收**：真实小程序码截图，采样后矩阵能在 SVG 中按扇形拼出可识别的小程序码

---

### 任务 5.4：小程序码 SVG 输出

**步骤**：
1. 在 `src/vector/shapes.rs`：
   ```rust
   /// 极坐标扇形路径
   pub fn polar_sector_path(
       cx: f64, cy: f64,
       r_inner: f64, r_outer: f64,
       theta_start: f64, theta_end: f64,
   ) -> String;
   ```
2. 在 `src/vector/svg.rs`：
   ```rust
   pub fn wx_grid_to_svg(grid: &WxGrid) -> String;
   ```
3. 每个采样点画一个小扇形（圆心角 = 360°/lines/2，宽度沿径向 = 单点厚度）

**文件**：`src/vector/shapes.rs`、`src/vector/svg.rs`

**验收**：SVG 在 Illustrator 打开看起来与原小程序码一致

---

### 任务 5.5：小程序码差异高亮 + UI

**步骤**：
1. `wx_grid_to_diff_preview_image`：把小程序码生成预览映射回原图二值图，按像素比较原图/生成图黑白差异
2. UI 中识别到 `CodeKind::WxMiniprogram` 时，掩膜面板隐藏（小程序码没掩膜），显示"重新采样"、"显示差异"和差异像素数
3. 状态栏写明红色=原图有生成图没有、蓝色=原图没有生成图有

**验收**：完整流程能跑通：粘贴小程序码 → 自动识别 → 矢量预览/差异预览 → 导出 SVG

---

## 阶段 6：抖音码 + SVG 文本剪贴板 + 打包

**完成记录（2026-06-02）**：
- 已实现 `src/detect/finder_dy.rs`：通过黑色连通域、圆度近似、同心嵌套和角落模板扫描检测抖音码左上、左下、右下三同心圆定位点；选点时加入 TL/BL/BR 方向约束，避免把小程序码 TL/TR/BL 三牛眼误判为抖音码
- 已在 `src/detect/mod.rs` 接入整体判型：结合 QR finder、小程序码/抖音码三点候选、彩色徽标和极坐标纹理签名，避免抖音码误识别成 QR，也避免小程序码误识别成抖音码
- 已新增 `src/codec/dy_grid.rs`：根据三定位点推算圆心、内外半径、边框状态、编码环和编码每环点数，并按极坐标采样得到 `DyGrid`；黑框版拆成 3、4 或 5 条编码环、1 个两段式外黑框和 2 条独立细环；前三条编码环固定存在，第 4/5 条候选内环按原图黑点密度、run 数、平均 run 长度和最长 run 长度逐层启用，避免中心 Logo 或徽标长弧被误判成编码环；外黑框按 `samples/两段外黑框.svg` 的 4 条切线生成两段完整扇环，其中 3 条可变切线用原图黑白边缘连续搜索；细环使用原图 Otsu 二值图按 720 角、5x7 专用采样核采样，120 点黑框版会在闭合前删掉徽标边缘 4 点以内短 run，再做小白缝闭合；右上徽标保留区内编码环保持空白，黑框版编码环不避让三牛眼白底，黑框版 72/120 点编码环使用不同徽标保留半径，120 点最外编码环额外剪掉紧邻徽标保留区的短 run，但不再做 7x5 宽核编码补采；细环按 72/120 点版本使用不同徽标避让半径；黑框版点数在 72/120 中评分选择，编码环采样和 SVG 导出都使用标准相位并按三牛眼对角线补偿整图旋转，`samples/黑框版2.jpg` 锁定为 120 点/编码环，旧 4 条标准编码环与 `samples/黑框版2.svg` 的 `g#c` 标准点位逐点一致；无框版固定 120 点/环、6 环
- 已接入应用流程和 UI：导入抖音码后先用原始三定位点把图像转正，再在转正后的彩色图和二值图上检测参数、采样、生成 SVG 和右侧预览；抖音面板提供重新采样、显示差异、可见总环数、编码每环点数、边框状态和差异像素数；黑框版可见环数按编码环 + 2 条细装饰环统计，外黑框不计入环数，本地标准样本覆盖 5/6/7 环
- 已在 `src/vector/svg.rs` 支持抖音码 SVG、光栅预览和像素级差异预览；同环连续黑点会先合并成 run，黑框版把两段外黑框和两条细环输出到 `g#a`，72 点和 120 点固定 `g#b` 由同一套内置布局参数生成，不再 `include_str!` 或复用样本 SVG 整组内容，并在黑框徽标内圆追加白底，再把实际采样的 3、4 或 5 条编码环输出到最顶层 `g#c`；无框版输出封闭填充圆角弧条，单个独立点输出真实 `<circle>`；旧右上角徽标为黑外圈、白内圈，并嵌入从新版 `samples/黑框版1.svg` 提取后内置的三色 Douyin logo path
- 黑框版右上固定徽标现在支持旧 logo 和从 `samples/黑框版另一种徽标样式.svg` 提取后内置的 bullseye 两种样式；采样时按形状局部搜索“黑中心、白间隔、黑同心环”签名选择样式，不依赖颜色，因为旧 logo 可能是彩色、灰度或近黑；SVG 输出和校正预览都使用同一个 `badge_style`，动态外框、细环、编码环和 bullseye 徽标统一使用 `#000`，旧 logo 徽标保留 `#fa1e5c`、`#5ffdff`、`#000` 三色
- 已完成打包优化：`[profile.release]` 启用 `opt-level = "z"`、LTO、单 codegen unit、strip 和 `panic = "abort"`

**验证已通过**：
- `cargo fmt --check`
- `cargo clippy --no-deps -- -D warnings`
- `cargo test`（包含 `samples/黑框版*`、`samples/无框版*` 抖音标准样图回归；样本 SVG 存在时，会额外读取 `samples/黑框版1.svg`、`samples/黑框版2.svg` 做固定布局和标准 SVG 逐点对比，但运行时代码不依赖这些文件）
- `cargo build --release`，`target/release/qracer.exe` 体积 5,431,296 bytes（约 5.18 MiB，低于 15MB 目标）

**调参记录（2026-06-02）**：
- 判型从“优先 QR / 单看三牛眼”改为综合彩色徽标和极坐标纹理签名，因为 QR 可以有圆形角点，小程序码和抖音码牛眼形状接近，旋转样本会让单点结构判断不稳定
- 边框检测增加外侧黑度保护：外圈黑度要足够高，同时 `r_max * 1.06` 外侧不能过黑，因为无框版暗背景会被旧规则误判为黑框
- 黑框版点数从固定 72 改为在 72/120 两个候选中评分选择，并把 `黑框版2.jpg` 回归锁定为 120 点，因为用户提供的标准 SVG 明确该样本是 120 点，黑框版实际存在两种点数
- 黑框版环数/半径从径向 profile 改为使用标准几何：内部主网格最多 5 条编码环，前三条固定存在，后两条候选内环按原图编码短 run 形态逐层启用，所以对外可显示 5/6/7 环；两条细环拆为 `DyDecorativeRing`，参与对外可见环数统计但不参与编码和 72/120 点数判断；外黑框拆为 `DyOuterFrame` 两段完整扇环，不计入环数
- 黑框版编码环相位不再由径向黑度峰值直接决定。点数评分仍搜索相位，但最终编码采样和 SVG 导出都使用标准 SVG 固定相位（72 点旧布局 `5°`、120 点旧布局 `3°`、bullseye 徽标布局 `2.5°`），并按三牛眼对角线补偿整图旋转；原因是黑度搜索会被 JPEG、抗锯齿和徽标边缘拉偏 0.x°，采样/导出相位不一致会在 Illustrator 重叠时造成徽标附近缺口
- 应用流程在进入抖音参数检测和采样前，会先按原始三定位点把原图转正：TL/BL/BR 三个小同心圆和推算出的右上角组成源四点，warp 到标准 TL/BL/BR 目标位置，再从转正图重新预处理和采样。这样做是因为黑框版 4 这类轻微旋转样本不是单个编码点阈值问题，而是整张图仍在原始坐标系内；先转正能让参数检测、徽标样式识别、采样和差异预览都使用同一标准坐标系，避免 Illustrator 重叠时出现编码环整体偏移 0.x°
- 黑框版采样阈值从 `>= 0.45` 改为 `>= 0.34` 的 3x3 投票，并在采样后删除弱前导端点：`3/9` 黑度的前导点仍删除，`4/9` 前导点只有在后续黑段不超过 2 格时删除；原因是 `>= 0.45` 会漏掉标准里的真实弱端点，而单纯低阈值会把 JPEG 边缘毛刺采成多余黑点。`黑框版4.jpg` 左下漏点属于 `4/9` 但后接长黑段，应保留；`samples/黑框版2.jpg` 的标准多采点只形成短碎段，仍应删除。无框版仍保留 `>= 0.26`
- 右上徽标检测增加右上象限、半径和到圆心距离约束，因为暗背景可能形成巨大暗色连通域，旧规则会把几乎所有采样点划入保留区，导致 SVG 只剩徽标；黑框版右上固定徽标保留区改为按三定位点估算，并取小牛眼外径约 2.5 倍，因为暗色连通域经常抓到内部 logo 而不是外侧大徽标
- 外黑框从“720 角采样装饰环”改为“两段完整外框”：`samples/两段外黑框.svg` 指出外黑框由 4 条径向切线限定，右上徽标大圆处绿色切线固定，另外 3 条切线在标准角附近用原图 Otsu 黑白边缘做 0.125° 级连续搜索微调；后续把前后探针从 1.5° 缩小到 0.5°，因为 `黑框版2.jpg` 对比 `黑框版2.svg` 时 1.5° 会把左侧切线推到黑白过渡之前，0.5° 更贴近实际边缘
- 两条细环继续从原图 Otsu 二值图按 720 角采样，而不是使用经过 3x3 开闭运算清理后的二值图，也不按 72/120 数据格采样；采样时避开右上徽标区域。细环判黑改为 5 个角向列、7 个径向偏移的专用核：同列任一径向命中即算列命中，至少 2 列命中或整体黑度 `>= 0.10` 判黑；120 点黑框版会先删除徽标边缘距离比例 `1.04..=1.14`、长度不超过 4 个 720 角采样点的短黑 run，并修剪能被 6 点小白缝闭合桥接到该短 run 的相邻尾点；之后填补不超过 6 个采样点的小白缝、删除短于 2 个采样点的孤立黑噪。这样可以补上 `黑框版2.jpg` 顶部细环的抗锯齿漏采，同时避免 `黑框版3新问题.svg` 中徽标圆框边缘被当作细环并被白缝闭合连成整段；`1.14` 上限避免新增样图中 `1.20+` 的真实细环端点被徽标边缘误剪。编码环仍使用清理后的二值图以降低 JPEG 噪声
- 徽标附近采样优化（2026-06-03）：`黑框版1.jpg` 中两条细环实际贴到徽标圆框，72 点黑框细环的徽标避让半径放宽到 `0.80 * badge.radius`；但 120 点 `黑框版2.jpg` 若全局缩小避让区会把徽标圆框边缘采成额外细环，因此 120 点细环仍使用 `1.04 * badge.radius` 严格避让，并在闭合白缝前额外删除徽标边缘 4 点以内短 run。`黑框版3新问题.svg` 暴露了此前只删编码 point97 仍会看起来没变的原因：蓝圈位置还被 `g#a` 细环短 run 和闭合桥接尾点压到；短 run 先删、桥接尾点同步剪掉后，黑框版3蓝圈不再与最终动态输出重叠，同时不误剪 `黑框版2.svg` 里的 46 点长标准细环段。用户进一步指出红圈位置原图应有两个编码点；按标注坐标映射后对应 ring0 point96 和 point97，因此编码短 run 剪枝不能再删除 point97。`黑框版3采样问题标注-01.svg` 标出此前强黑度邻接补采生成的两个最外层编码点实际落在徽标扇形内；后续 `黑框版2漏采点标注.svg`、`黑框版3多采漏采点位标注.svg`、`黑框版3新问题.svg` 和 `黑框版4多采漏采点位标注.svg` 进一步显示 120 点编码环在徽标附近既有红圈真实点也有蓝圈误采。编码环改为黑框版和无框版都在右上徽标保留区内保持空白，不再补采；120 点黑框版徽标保留区改为分环处理：最外编码环仍使用 `1.12 * badge.radius` 抑制外圆框边缘误采，内侧编码环使用 `1.04 * badge.radius`，避免 `黑框版9漏采点标注.png` 中 ring1 point107 这类贴徽标但在外侧的真实编码点被误删。`黑框版9多采点标注.png` 进一步确认 bullseye 徽标下沿 ring0 point109 虽有 `5/9` 黑度，但距离比例约 `1.011`，属于贴徽标边缘假点，应继续由最外环 `1.12` 规则清空；point110 距离比例约 `1.157`，才是后续真实编码段起点。72 点黑框版编码环和无框版使用 `1.04 * badge.radius`，避免 `黑框版1漏采点标注.svg` 中靠近徽标的里层真实编码点被误删。120 点黑框版最外编码环还会处理紧邻徽标保留区、长度不超过 2 个 cell、最近徽标距离比例在 `1.20..=1.45` 的短 run，但只删除其中距离比例 `<= 1.20` 的内部徽标边缘 cell；这样可以同时保留 `黑框版3新问题.svg` 红圈对应的 ring0 point96/point97，并由细环短 run 剪枝解决蓝圈误采，不误删 `黑框版2.svg` 标准里的 3-cell 外层 run。细环是装饰环，不套用编码环必空规则
- 72 点模糊徽标边缘优化（2026-06-04）：`黑框版5.jpg` 的原图较模糊，右上徽标圆框外缘会在最外编码环生成贴徽标 gap 的 2-cell 假短 run，也会在第 2 条细环留下残段。72 点最外编码环现在只在徽标 gap 边界检查距离比例 `1.04..=1.10` 的短 run，并按单 cell 比例 `<= 1.08` 删除更贴徽标的假点，不再整段删除；72 点细环在闭合白缝前后都按徽标边缘带清理，并允许边缘带外 run 端点做 1 点弱补采。这里不把 72 点细环避让半径整体收紧，因为 `黑框版1.jpg` 的两条细环确实需要贴到徽标下方，局部清理能去掉 `黑框版5多采点标注.svg` 蓝圈误采；`黑框版5漏采点标注.svg` 进一步显示旧的整段删除和 `1.34` 细环上限会误删强黑真实点，因此改为按 cell 删除并收窄细环清理带。`samples/黑框版4.jpg` 的新标注又暴露了另一面：第 2 条细环在徽标边缘清理带内有两段真实强黑线被切断。为避免回到全局放宽造成的徽标圆框残段，现在只桥接第 2 条细环中不超过 10 个 720 角采样点的 gap，且要求 gap 前后都有已采样细环 run、gap 内仍落在徽标边缘带、每点 5x7 核至少 5 个角向列和 18 个黑采样命中
- 根目录新增黑框样图优化（2026-06-04）：多张 120 点黑框样图在徽标附近的细环端点被旧 `1.04..=1.26` 清理带误剪，尤其是 `黑框版4.jpg` 徽标上沿外细环端点；120 点细环短 run 清理带收窄为 `1.04..=1.14`，继续清理贴徽标圆框的短误采，但让 `1.20+` 的外侧真实细环端点参与白缝闭合。`黑框版4.jpg` 左下漏采编码点是 `4/9` 黑度的前导 cell，但后面连续多个强黑 cell；`samples/黑框版2.jpg` 的标准多采点同为 `4/9`，但只形成 2-cell 短碎段。弱前导剪枝因此改为按 run 长度区分：`3/9` 前导点仍删除，`4/9` 前导点仅在 run 长度不超过 2 时删除
- 黑框版右上徽标样式选择（2026-06-04）：新增从 `samples/黑框版另一种徽标样式.svg` 提取后内置 path 的 bullseye 固定徽标候选。旧 logo 不一定有彩色像素，因此不能用颜色判断；当前在三定位点估算的徽标附近搜索同心形状签名，比较中心小黑点、外侧白间隔和黑同心环的暗度比例。只有三项同时满足阈值才选择 bullseye，否则仍使用旧 logo 样式；局部搜索可以容忍黑框几何估算的徽标中心偏差，而不影响编码环保留区半径
- 黑框版编码环标准相位修正（2026-06-04）：采样和 SVG 导出 `g#c` 编码环都按标准 SVG 布局使用固定相位：72 点旧布局 `5°`、120 点旧布局 `3°`、bullseye 徽标布局 `2.5°`，再由三牛眼对角线补偿整图旋转。原因是黑点搜索会被抗锯齿/JPEG/徽标边缘和码点黑度分布拉偏 0.x°，在 Illustrator 中与原图重叠时会表现为三牛眼准确但编码环整体微旋；此前只固定导出相位仍会让右上徽标附近的采样布尔点和最终 SVG 点位错开
- 120 点黑框版徽标外侧弱边缘补采撤销（2026-06-04）：后续蓝圈标注显示，原 7x5 编码环宽核补采会在离徽标并不近的真实白格上被相邻黑格抗锯齿触发。编码环是离散 cell，误补一个白格比漏掉弱边缘更破坏码点一致性，因此撤销该补采，继续使用 3x3 编码采样、标准相位、分环徽标保留区和短 run 剪枝；5x7 宽核只保留给两条连续细环。`黑框版9多采点标注.png` 进一步说明该位置也不是宽核问题，而是贴 bullseye 徽标边缘的 point109 被误当作编码点；最外环保留区维持 `1.12`，去掉蓝框多采，同时 point110 以后的连续编码段仍采黑。`黑框版9漏采点标注.png` 显示内侧 ring1 point107 为满黑真实点，不能被同一个 `1.12` 圆形保留区吞掉，因此内侧编码环保留区收回到 `1.04`
- 黑框版中心 Logo 边界采样优化（2026-06-05）：`黑框版11漏采点标注.png` 的最内编码环红圈实际包含 ring4 point53 和 point54 两个满黑编码点；point53 距离中心 Logo 半径比例约 `0.983`，point54 约 `1.005`。黑框版编码环位于最顶层，中心 Logo 保留区若继续使用 `1.02` 或 `1.00` 会分别漏掉两个点或漏掉 point53，因此黑框版中心 Logo 编码保留区收窄到 `0.98 * logo.radius`；无框版仍保持 `1.02`，避免把中心 Logo 内容采成码点
- 黑框版 5/6/7 环检测修正（2026-06-05）：用户补充黑框版存在 5 环、6 环、7 环，环数是编码环 + 两条细环的总和，外黑框不算；因此内部编码环数应为 3/4/5，而不是按 120 点旧音符或徽标样式固定。采样阶段现在先识别徽标样式和中心 Logo 保留区，再对第 4、5 个候选编码环做 3x3 采样并计算黑点密度、黑 run 数、平均 run 长度和最长 run 长度。真实编码环应有较多离散短段；`samples/黑框版6/8/9.png` 的候选内环为全黑、全白或少数长弧，属于 Logo/空白干扰，应判为 5 环；`samples/黑框版2/3/4.jpg` 判为 6 环，`samples/黑框版1.jpg` 判为 7 环
- 牛眼附近采样优化（2026-06-03）：`黑框版4.jpg` 标出左上牛眼附近最外层编码环漏采点 `(ring0,75)`；因为黑框版 SVG 的编码环 `g#c` 在固定牛眼 `g#b` 之上，黑框版编码环采样不再把三牛眼白底作为保留区。无框版绘制顺序不同，仍按现有三牛眼保留区跳过
- 黑框版 SVG 徽标白底优化（2026-06-03）：标准 `g#b` 的徽标外圈 path 内孔本身是透明的，72 点细环贴到徽标外框后会有少量伸进内圆；导出时在徽标黑外圈 path 后、logo path 前追加白色内圆，保证下层 `g#a` 细环不会从徽标内圆透出来
- SVG 输出改为按同环 run 合并；黑框版按点数选择布局：72 点使用 `黑框版1.svg` 的画布/中心和完整 `g#b`，120 点使用 `黑框版2.svg` 的画布/中心并从 `g#b` 删除前两个外框 path；两段外黑框和细环输出到 `g#a`，再绘制固定 `g#b`，最后把采样编码环输出到最顶层 `g#c`，因为外框/细环需要被牛眼/徽标白底遮挡，但编码环不能被牛眼白底遮挡；无框版不使用描边，连续 run 用闭合圆角填充路径，独立点用圆，因为黑框非编码视觉环仍需还原原图，无框版端点则应为圆端
- 后续若继续优化抖音码采样，必须同步更新 `docs/ARCHITECTURE.md` 的算法说明、本节调参记录或决策日志，并写明触发样本、失败原因、采用的新规则及取舍

### 任务 6.1：抖音码 Finder（三同心圆）

**步骤**：
1. `src/detect/finder_dy.rs`：
   ```rust
   pub struct DyFinder {
       pub cx: f64,
       pub cy: f64,
       pub rings: Vec<f64>, // 每层嵌套圆半径
   }
   
   pub fn find_dy_finders(bin: &BinaryImage) -> Vec<DyFinder>;
   ```
2. 算法：复用 5.1 的轮廓 + 圆度 + 嵌套检测，但抖音码定位点是左上、左下、右下，右上角大圆为徽标

**验收**：真实抖音码截图，找到 3 个同心圆定位点

---

### 任务 6.2：抖音码环数 + 点数检测

**步骤**：
1. `src/codec/dy_grid.rs`：
   ```rust
   pub struct DyParams {
      pub ring_count: u8,        // 内部编码环数：黑框 3/4/5 条编码环 / 无框 6 环
       pub points_per_ring: u32,  // 72 / 120
       pub has_border: bool,      // 黑框版 / 无框版
   }
   
   pub fn detect_dy_params(
       bin: &BinaryImage,
       finders: &[DyFinder; 3],
   ) -> crate::error::Result<DyParams>;
   ```
2. 算法：
   - 先按 TL/BL/BR 排序三定位点，圆心取 TL 与 BR 的中点；结合三定位点外半径估算 `r_max`，再按比例得到 `r_min`
   - 边框检测：在 `r_max * 0.88..1.0` 处做 360 点径向黑度评分，并要求 `r_max * 1.06` 外侧黑度低，避免无框版暗背景误判为黑框
   - 点数：黑框版在 72/120 两个候选中做网格评分选择，`黑框版2.jpg` 回归断言为 120 点/环；无框版固定 120 点/环
   - 环数：对外显示为编码环 + 装饰细环的总和，外黑框不计入；无框版固定 6 环，黑框版可显示 5/6/7 环。内部采样主网格为 3/4/5 条编码环，半径来自标准几何，并按三定位点距离缩放映射回输入图；前三条编码环固定存在，后两条候选内环按黑点密度和短 run 形态逐层判断
   - 外黑框/细环：黑框版外黑框不参与编码和点数判断，单独存入 `DyOuterFrame`，按 4 条切线生成两段完整扇环，除右上徽标切线外的 3 条切线按原图边缘微调；两个细环单独存入 `DyDecorativeRing` 并按原图 720 角、5x7 专用核采样输出；点数和相位检测只使用前三条固定编码环

---

### 任务 6.3：抖音码采样 + SVG

**步骤**：
1. 应用层先用原始 TL/BL/BR 三定位点把抖音码图像转正，再在转正后的彩色图和二值图上调用 `detect_dy_params(...)`、`sample_dy(...) -> DyGrid` / `sample_dy_with_logos(...) -> DyGrid`；采样器根据已检测参数生成同心环规格，并做角向相位搜索。这样避免带旋转原图直接采样时编码环整体偏移
2. 每个 (ring, point) 使用 3x3 极坐标采样投票；黑框版编码采样和 SVG 导出都使用标准相位（72 点 `5°`、120 点旧 logo `3°`、120 点 bullseye `2.5°`，按三牛眼对角线补偿旋转），黑度比例 `>= 0.34` 判黑，并删除弱前导端点毛刺：`3/9` 前导点删除，`4/9` 前导点只有后续黑段不超过 2 格时删除；无框版 `>= 0.26` 判黑；右上徽标和中心 Logo 覆盖区作为保留区跳过，黑框版编码环不跳过三牛眼白底，无框版仍跳过三牛眼。黑框版和无框版编码环在右上徽标保留区内一律保持空白，不做徽标内部强黑度补采；其中 120 点黑框版最外编码环徽标保留半径为 `1.12 * badge.radius`，内侧编码环为 `1.04 * badge.radius`，72 点黑框版和无框版也为 `1.04 * badge.radius`；黑框版中心 Logo 编码保留半径为 `0.98 * logo.radius`，无框版仍为 `1.02 * logo.radius`；120 点黑框版最外编码环会额外处理紧邻徽标保留区的 2-cell 短 run，但只删除其中距离比例 `<= 1.20` 的内部徽标边缘 cell；编码环不做 7x5 宽核补采，避免相邻黑格抗锯齿把真实白格补成黑格；72 点黑框版最外编码环会删除贴徽标 gap、距离比例 `1.04..=1.10` 且单 cell 比例不超过 `1.08` 的 2-cell 假短 run
3. 注意：黑框版外黑框不是采样碎片，而是两段完整外框；右上徽标切线固定，其余 3 条切线按原图黑白边缘连续搜索微调，当前使用 0.5° 前后探针。两个细环用原图 Otsu 二值图和 720 角采样保留细线，避开右上徽标区域，并做 5x7 细环核、小白缝闭合和短黑噪删除；72 点细环使用放宽徽标避让以贴到徽标圆框，并在闭合前后清理徽标边缘带（第 1 条 `1.04..=1.14`，第 2 条 `0.89..=1.22`），边缘带外的 run 端点允许 1 点弱补采，防止模糊徽标圆框残段被闭合回去但不误删真实细环端点；120 点细环保持严格避让以匹配 `黑框版2.svg`，并在闭合白缝前删除徽标边缘距离比例 `1.04..=1.14`、4 点以内短 run，防止徽标圆框被接成细环，同时保留新增样图中外侧真实细环端点；编码环用清理后的二值图
4. SVG：同一环连续黑点先合并成 run；黑框版把两段外黑框和细环输出到 `g#a`，按 72/120 布局追加对应标准 SVG 的固定 `g#b`，最后把实际采样到的 3、4 或 5 条码点输出到最顶层 `g#c`，让外框/细环被白底遮挡但编码环不被白底遮挡；无框版输出闭合填充圆角弧条，单个独立点输出 `<circle>`；全程不用描边
5. 徽标：黑框版右上徽标和三牛眼由代码内置布局参数生成，外黑框 path 不复用；旧 logo 样式使用从新版 `samples/黑框版1.svg` 提取后内置的三色 Douyin logo path，导出时在黑框徽标黑外圈后追加白色内圆，再绘制 logo path，防止下层细环透进徽标内圆；bullseye 样式使用从 `samples/黑框版另一种徽标样式.svg` 提取后内置的徽标 path 和 `viewBox 0 0 626.65 628.84`。样式选择按局部同心形状签名判断，不依赖颜色；无框版右上徽标为黑外圈、白内圈，并嵌入同一组三色 logo path；徽标检测失败时根据三定位点估算位置。抖音码动态 path 和 bullseye 徽标统一输出 `#000`，旧 logo 徽标保留 `#fa1e5c`、`#5ffdff`、`#000`

**验收**：完整流程跑通；应用层对抖音码先生成转正图再采样；黑框版对外显示编码环 + 2 条装饰细环的总数（外黑框不计入），其中 `samples/黑框版1.jpg` 为 7 环、`samples/黑框版2/3/4.jpg` 为 6 环、`samples/黑框版6/8/9.png` 为 5 环，状态栏显示“编码每环 N 点”；`samples/黑框版2.jpg` 的旧 4 条标准编码环 4x120 采样布尔网格与 `samples/黑框版2.svg` 的 `g#c` 逐点一致；`samples/黑框版2.svg` 的 `g#a` 会被解析成 720 角标准覆盖，用于约束两条细环 run 数和角段覆盖，`g#b` 前两个外框 path 会被解析成标准切线角度，用于约束外框切线误差不超过 2°；`samples/黑框版1.jpg` 输出由内置布局生成固定 `g#b`，但不固定复用 `samples/黑框版1.svg` 的 `g#a`；`samples/黑框版另一种徽标样式.jpg` 必须按形状判为 bullseye 徽标样式，输出从该 SVG 提取后内置的 bullseye 徽标 path、三牛眼结构和非正方形 viewBox，且旧 `logo.svg` path 不应出现在该黑框输出里，旧布局深色 `#231815` 和 bullseye fixture 深色 `#221714` 都不应出现在抖音黑框输出里；外黑框输出为两段完整外框，两条细环能从原图采到且不会过度碎片化，保留 5 条编码环的样本最内环能从原图采到黑点；`黑框版1.jpg` 细环能采到徽标外框接触段，`黑框版1漏采点标注.svg` 中靠近徽标的 72 点里层编码环能采回，`黑框版2漏采点标注.svg`/`黑框版3多采漏采点位标注.svg`/`黑框版3新问题.svg`/`黑框版4多采漏采点位标注.svg` 中红圈半径内所有编码 cell 为黑且由最终 `g#c` 编码环 run 覆盖、蓝圈不与编码环/细环/外框动态输出重叠，`黑框版5多采点标注.svg` 的蓝圈最近动态 cell 不为黑，`黑框版5漏采点标注.svg` 的红圈最近编码/细环动态 cell 能采回并由最终 run 覆盖，根目录 `黑框版4.jpg` 左上牛眼附近编码点不会被牛眼白底保留区误删、`黑框版9多采点标注.png` 的 bullseye 徽标下沿最外环假编码点保持空白、`黑框版9漏采点标注.png` 的内侧 ring1 point107 真实编码点能采回、`黑框版11漏采点标注.png` 的最内编码环真实点不会被中心 Logo 保留区误删、徽标上沿 120 点外细环端点不会被误剪、左下方向 `4/9` 但后接长黑段的编码起点能保留，所有黑框版和无框版编码环在右上徽标保留区内保持空白

---

### 任务 6.4：SVG 文本剪贴板

**目标**：把当前生成的 SVG 文本直接写入剪贴板；Illustrator 可直接把 SVG 代码粘贴为矢量对象，粘贴结果与导出 SVG 使用同一份字符串。

**实现状态（2026-05-31）**：已按用户要求跳过 6.1-6.3，先实现 6.4。此前尝试的 EMF 路径已移除；当前 `复制到剪贴板` 直接用 `arboard::Clipboard::set_text()` 写入 `last_svg`，不再维护 Win32 EMF 生成代码，也不再改写小程序码尺寸。用户手测确认：将 SVG 代码文本粘贴到 Illustrator 会直接绘制矢量图。

**步骤**：
1. 复用已有 `arboard` 依赖，无需新增 Windows 剪贴板 crate
2. `QRacerApp::try_copy_vector()`：
   - 若 `last_svg` 为空，提示没有可复制的 SVG
   - 否则把 `last_svg` 原文写入剪贴板文本
3. 工具栏"复制到剪贴板"按钮在 `last_svg.is_some()` 时启用

**文件**：`src/app.rs`、`src/ui/toolbar.rs`

**验收**：
- [x] 工具内"复制到剪贴板"按钮接通 QR / 小程序码 SVG 文本写剪贴板
- [x] 手动验证：SVG 代码文本粘贴到 Illustrator 后直接绘制矢量图
- [x] 粘贴结果与导出 SVG 使用同一份 SVG 文本

---

### 任务 6.5：打包优化

**实现状态（2026-06-02）**：已完成。当前 release 产物为 `target/release/qracer.exe`，体积 5,431,296 bytes（约 5.18 MiB），低于 15MB 验收目标。

**步骤**：
1. `Cargo.toml` 末尾追加：
   ```toml
   [profile.release]
   opt-level = "z"
   lto = true
   codegen-units = 1
   strip = true
   panic = "abort"
   ```
2. `cargo build --release`
3. 记录 `target/release/qracer.exe` 体积（目标 < 15MB）
4. 可选：UPX 压缩到 ~7MB

**验收**：release exe < 15MB；启动 < 1s；功能与 debug 版本一致

---

## 阶段 7（部分前置完成）：后台任务运行器

**触发条件**：用户报告粘贴/打开后 UI 等待结果期间没有反馈，容易误以为软件卡死。

**当前完成（2026-05-31）**：
- 已在 `QRacerApp` 内用轻量 `std::sync::mpsc` + `std::thread` 接入后台处理：导入图片后立即显示左侧原图，右侧预览区和状态栏显示 loading；后台完成后一次性回填码类型、校正图、SVG、差异数和预览
- 已新增 `src/screen_capture.rs`：点击"截屏"后最小化主窗口，显示 Win32 全屏透明遮罩；用户拖拽框选区域后用 GDI `BitBlt` 捕获并按普通图片导入流程处理
- 已修复截屏框选后主窗口不恢复的问题：不再隐藏主窗口，改为最小化以保留任务栏入口；截屏线程结束时主动唤醒 egui、排队取消最小化，并用 Win32 `ShowWindow(SW_RESTORE)` 做原生还原兜底
- 当前没有单独创建 `src/job/runner.rs`，因为后台任务类型仍少；任务 6.4 的 SVG 文本写剪贴板耗时短，仍同步执行。阶段 6/7 若增加批量处理、取消任务或更长耗时操作，再抽出统一 runner

**后续可选步骤**：
1. 新建 `src/job/runner.rs`，统一 `Job` / `JobResult` / 取消令牌
2. 把掩膜重算、网格像素匹配等耗时操作迁入后台任务
3. 为长任务增加真实阶段进度，而不是当前的循环 loading

---

## 阶段 8：QR 外观样式切换与自动推断

**实现状态（2026-06-07）**：已完成。

**目标**：标准 QR 在完成识别、校正和矩阵生成后，允许用户在标准、微信样式、小红书样式三种 SVG 外观之间切换；首次结果尽量根据原图自动选择外观，但手动切换始终保留。

**已实现内容**：
- `src/vector/svg.rs` 新增 `QrAppearance::{Standard, Wechat, Xiaohongshu}`，并提供 `qr_matrix_to_svg_with_appearance()` 和 `qr_matrix_to_preview_image()`；标准样式保持方块矩阵，微信样式使用圆角点、圆角 finder 和中心徽标，小红书样式使用圆点和圆形 finder。
- 微信中心徽标不再使用手写近似图形，已把 `samples/微信样式.svg` 中的黑色 logo path 提取为常量，导出时按当前 QR 画布缩放；参考 SVG 中用于对齐的白色网格不会输出。
- 小红书预览修正为跳过三个 QR finder 区域内的矩阵点，再绘制圆形 finder，避免预览角落出现多余像素圆点；差异高亮仍作为顶层红/蓝矩形覆盖。
- `src/app.rs` 在 QR finder 选定并完成校正后推断初始外观：微信样式需要 QR 几何中心存在深色 badge 且周围有白色清空边带；小红书样式在校正后的 QR 坐标中同时检查 finder 角落留白、边缘黑环和中心黑块，避免透视拍照的标准 QR 被原图角点单采样误判；否则使用标准样式。这样普通标准 QR 的中心数据块不会被误判为微信样式，拍照标准 QR 也不会因 finder 角点采样偏移被误判为小红书样式。
- `src/ui/compare_view.rs` 在“校正预览”标题右侧显示 `标准 / 微信样式 / 小红书样式` 切换按钮。按钮只在当前结果是 QR 且已有矩阵时显示；点击后重建 `last_svg` 和右侧预览。
- 判型层 `detect/mod.rs` 先用 QR lattice 签名确认真实 QR，避免微信/小红书特殊外观 QR 被小程序码或其它圆形码路径抢走。

**回归验证**：
- `samples/标准样式.jpg` 自动推断为 `Standard`。
- `samples/标准样式拍照.jpg` 自动推断为 `Standard`。
- `samples/微信样式.png` 自动推断为 `Wechat`。
- `samples/小红书样式.jpg` 自动推断为 `Xiaohongshu`。
- `samples/微信样式` 与 `samples/小红书样式` 判型必须返回 `CodeKind::Qr`。
- SVG 输出不包含参考 SVG 的白色网格；微信输出包含内置 logo path；小红书预览 finder 角落无多余圆点。

---

## 全局质量门

每个 PR 合并前必须满足：

- [ ] `cargo build --release` 通过
- [ ] `cargo clippy --no-deps -- -D warnings` 通过
- [ ] `cargo fmt --check` 通过
- [ ] `cargo test` 通过
- [ ] 没有新增不必要的 `unsafe` 块
- [ ] 阶段 2 起每个新公开函数有 doc comment
- [ ] 阶段 3 起的每张 fixture 截图能跑通端到端流程

---

## 附录 A：依赖速查

| crate | 用途 | 引入阶段 |
|---|---|---|
| `eframe` / `egui` / `egui_extras` | GUI | 1 |
| `image` | 图像 IO | 1 |
| `arboard` | 剪贴板读图；SVG 文本写剪贴板 | 1 / 6.4 |
| `windows-sys` | Windows 前台 `Ctrl+V` 图片粘贴快捷键检测；截屏遮罩和 GDI 屏幕捕获 | 1 |
| `rfd` | 文件对话框 | 1 |
| `anyhow` | 应用层错误 | 1 |
| `imageproc` | 二值化、形态学、轮廓 | 2 |
| `nalgebra` | 单应矩阵 | 2 |
| `thiserror` | 库层错误 | 2 |
| `rxing` | QR 解码 | 3 |
| `qrcodegen` | QR 生成（指定掩膜） | 3 |

## 附录 B：术语表

- **Finder Pattern**：QR 三角的"靶心"定位方块（1:1:3:1:1）
- **掩膜（Mask）**：QR 编码末尾用 8 种位运算之一异或数据区，0-7
- **ECC（Error Correction Code）**：QR 纠错等级 L/M/Q/H
- **单应矩阵（Homography）**：3×3 矩阵，描述两个平面间的透视变换
- **DLT**：Direct Linear Transform，从 4 对点对应求单应矩阵的线性方法
- **Otsu**：图像二值化的自动阈值法
- **牛眼**：小程序码定位点的俗称
- **装饰环**：抖音码中不参与编码但仍需按原图/几何规则输出的视觉环；当前黑框版外黑框是两段式 `DyOuterFrame`，两条细环是 `DyDecorativeRing`

---

## 附录 C：抖音码右上徽标与细环优化计划（2026-06-05）

根据 `samples/抖音码简介.pdf`，黑框版第 1、3 环是装饰细环，不参与编码；右上角大圆是第 4 个辅助定位点。当前问题集中在右上徽标附近细环漏采/误采，因此后续实现按以下顺序推进：

1. **右上徽标作为第四定位点**：三定位点粗校正后，在右上预估区域拟合真实徽标外圆，使用真实右上圆心替代 `tl + br - bl` 推算点参与二次透视校正。
2. **细环模板化重建**：细环不再只依赖逐点黑度采样；以标准黑框几何生成两条装饰细环，远离保留区用图像确认，靠近右上徽标处用圆弧连续性和徽标圆边界裁切决定端点。
3. **真实徽标外圆边界**：徽标附近的编码环与细环保留/裁切优先使用拟合得到的徽标外圆，而不是只依赖固定比例半径；比例常量保留为兜底。
4. **细环灰度亚像素积分采样**：细环采样改用转正彩色图的灰度双线性/小扇区积分，降低单像素四舍五入和 Otsu 二值化在细线边缘造成的抖动。

验收重点：`黑框版4细环漏采标注.jpg` 红圈位置的两段细环应连续到徽标外圆附近；`samples/黑框版2/3/4.jpg` 和根目录 `黑框版4.jpg` 不应重新出现徽标圆框误采为细环的问题；编码环仍保持右上徽标保留区内空白。
