# QRacer 分步实施计划

> 文档版本：v1.0  
> 最后更新：2026-05-29  
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
- `src/ui/{toolbar, compare_view}.rs`：工具栏 + 左右对比

**验收已通过**：`cargo build` 无 warning；窗口启动正常；粘贴/打开能加载图像。

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
- 当前仓库没有 `assets/samples/` 真实截图样本；阶段 2 自动化验证使用 `qrcodegen` 合成 QR 样本
- 真实拍歪 QR 截图、小程序码/抖音码截图的 UI 手测需要在样本补齐后执行

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
- 已启用工具栏"导出 SVG"按钮：生成成功后可保存 `.svg` 文件；"复制矢量"仍留到阶段 6 的 EMF 剪贴板实现
- "网格兜底"按钮已作为阶段 4 占位接入，当前不执行采样兜底

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

**目标**：UI 上添加 0-7 掩膜单选 + "网格兜底"按钮，以及"自动尝试所有掩膜"按钮。

**步骤**：
1. 新建 `src/ui/mask_panel.rs`：
   ```rust
   pub fn show(ui: &mut egui::Ui, app: &mut QRacerApp);
   ```
   - 8 个掩膜单选（RadioButton）
   - "网格兜底"按钮（阶段 4 接入）
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

## 阶段 4：QR 网格兜底（已完成）

**完成记录（2026-05-29）**：
- 已新增 `src/codec/qr_grid.rs`：通过阶段 3 的 QR 版本推断能力识别模块数，并在校正图上按模块中心 3×3 多数投票采样生成 `QrMatrix`
- 已接通 `MaskChoice::GridFallback`：点击"网格兜底"后不再走 `qrcodegen` 重生成，而是直接采样校正图生成 SVG 和右侧预览
- 已支持"解码失败但版本可推断"场景：阶段 2 校正成功后，即使 QR payload 解码失败，UI 仍显示"网格兜底"入口
- 掩膜单选和"自动选最佳"在未解码时禁用；"网格兜底"只依赖校正图和版本推断

**验证已通过**：
- `cargo fmt --check`
- `cargo test`（23 个单元测试通过）
- `cargo build`
- `cargo clippy --no-deps -- -D warnings`

**仍需人工/样本验收**：
- 当前仍缺少真实损坏 QR 截图 fixture；自动化验证使用 `qrcodegen` 合成无噪 QR 并确认采样矩阵 1:1 等于原矩阵
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

### 任务 4.2：UI 接通网格兜底

**步骤**：
1. `MaskChoice::GridFallback` 分支：调用 `sample_qr_grid` 而非 `regenerate_qr`
2. 显示状态："使用网格兜底（保证 1:1 还原）"

**验收**：
- [ ] 点"网格兜底"按钮，preview 侧应与原图模块完全一致（差异 = 0）
- [ ] 用一张"无解码可能"的损坏 QR（手动把数据区涂掉几个模块），网格兜底仍能输出原样矢量

---

## 阶段 5：小程序码识别与采样

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
1. `compute_diff_wx`：同 QR 差异，但比较的是极坐标采样
2. UI 中识别到 `CodeKind::WxMiniprogram` 时，掩膜面板隐藏（小程序码没掩膜），只显示"重新采样"和导出

**验收**：完整流程能跑通：粘贴小程序码 → 自动识别 → 矢量预览 → 导出 SVG

---

## 阶段 6：抖音码 + EMF 剪贴板 + 打包

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
2. 算法：复用 5.1 的轮廓 + 圆度 + 嵌套检测，但抖音码定位点是 3 层嵌套同心圆（不像小程序码的牛眼是同心圆+点）

**验收**：真实抖音码截图，找到 3 个同心圆定位点

---

### 任务 6.2：抖音码环数 + 点数检测

**步骤**：
1. `src/codec/dy_grid.rs`：
   ```rust
   pub struct DyParams {
       pub ring_count: u8,        // 5 / 6 / 7
       pub points_per_ring: u32,  // 72 / 120
       pub has_border: bool,      // 黑框版 / 无框版
   }

   pub fn detect_dy_params(
       bin: &BinaryImage,
       finders: &[DyFinder; 3],
   ) -> crate::error::Result<DyParams>;
   ```
2. 算法：
   - 圆心 = 三定位点几何中心
   - 径向扫描：从圆心向外画线，记录黑白交替 → 环数
   - 角向扫描（在第 2 环上 360°）→ 黑白交替次数 = 点数
   - 边框检测：外圈外是否有 1-2 像素的黑框

---

### 任务 6.3：抖音码采样 + SVG

**步骤**：
1. `sample_dy(...) -> DyGrid`：每个 (ring, point) 极坐标采样
2. 注意：环 1 和环 3 是装饰环，不参与编码 → 但矢量化必须 1:1 保留（按原图采样）
3. SVG：每个采样点画一段圆环扇区（用 `polar_sector_path`）

**验收**：完整流程跑通

---

### 任务 6.4：EMF 剪贴板

**目标**：把 SVG 内容转成 Windows EMF，写入剪贴板，可在 Illustrator 粘贴为矢量。

**步骤**：
1. `Cargo.toml`：
   ```toml
   clipboard-win = "5"
   windows = { version = "0.59", features = [
       "Win32_Graphics_Gdi",
       "Win32_System_DataExchange",
       "Win32_System_Memory",
       "Win32_Foundation",
       "Win32_UI_WindowsAndMessaging",
   ] }
   ```
2. 新建 `src/clipboard/emf.rs`：
   ```rust
   /// 把 QR 模块矩阵 / 极坐标网格转 EMF 矢量并写剪贴板
   pub fn copy_qr_matrix_as_emf(matrix: &[Vec<bool>], module_mm: f64) -> crate::error::Result<()>;
   pub fn copy_polar_samples_as_emf(grid: &WxGrid_or_DyGrid) -> crate::error::Result<()>;
   ```
3. 实现（**所有 unsafe 集中在这里**）：
   1. `CreateEnhMetaFileW(NULL, NULL, ...)` 创建内存 EMF DC
   2. 在 DC 上调 `Rectangle` / `Polygon` 画矢量
   3. `CloseEnhMetaFile` → 返回 `HENHMETAFILE`
   4. `OpenClipboard` + `EmptyClipboard` + `SetClipboardData(CF_ENHMETAFILE, handle)` + `CloseClipboard`
4. 用 `clipboard-win` 的 RAII 包装（如有）减少 unsafe

**文件**：`src/clipboard/mod.rs`、`src/clipboard/emf.rs`

**验收**：
- [ ] 工具内点"复制矢量" → 切到 Illustrator → Ctrl+V → 出现矢量 QR（可点选每个方块）
- [ ] 验证缩放无失真（说明是矢量而非位图）

---

### 任务 6.5：打包优化

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

## 阶段 7（可选）：后台任务运行器

**触发条件**：用户报告"自动选最佳掩膜"或"复杂截图识别"卡顿（UI 冻结 > 500ms）

**步骤**：
1. 新建 `src/job/runner.rs`：
   ```rust
   pub struct JobRunner {
       sender: mpsc::Sender<Job>,
       receiver: mpsc::Receiver<JobResult>,
   }

   pub enum Job { ... }
   pub enum JobResult { ... }
   ```
2. 在工作线程跑识别/重生成
3. UI 每帧 `try_recv()`，并显示加载 spinner

---

## 全局质量门

每个 PR 合并前必须满足：

- [ ] `cargo build --release` 通过
- [ ] `cargo clippy --no-deps -- -D warnings` 通过
- [ ] `cargo fmt --check` 通过
- [ ] `cargo test` 通过
- [ ] 没有新的 `unsafe` 块在 `src/clipboard/emf.rs` 之外
- [ ] 阶段 2 起每个新公开函数有 doc comment
- [ ] 阶段 3 起的每张 fixture 截图能跑通端到端流程

---

## 附录 A：依赖速查

| crate | 用途 | 引入阶段 |
|---|---|---|
| `eframe` / `egui` / `egui_extras` | GUI | 1 |
| `image` | 图像 IO | 1 |
| `arboard` | 剪贴板读图 | 1 |
| `windows-sys` | Windows 前台 `Ctrl+V` 图片粘贴快捷键检测 | 1 |
| `rfd` | 文件对话框 | 1 |
| `anyhow` | 应用层错误 | 1 |
| `imageproc` | 二值化、形态学、轮廓 | 2 |
| `nalgebra` | 单应矩阵 | 2 |
| `thiserror` | 库层错误 | 2 |
| `rxing` | QR 解码 | 3 |
| `qrcodegen` | QR 生成（指定掩膜） | 3 |
| `clipboard-win` | Win 剪贴板 | 6 |
| `windows` | Win32 GDI（EMF） | 6 |

## 附录 B：术语表

- **Finder Pattern**：QR 三角的"靶心"定位方块（1:1:3:1:1）
- **掩膜（Mask）**：QR 编码末尾用 8 种位运算之一异或数据区，0-7
- **ECC（Error Correction Code）**：QR 纠错等级 L/M/Q/H
- **单应矩阵（Homography）**：3×3 矩阵，描述两个平面间的透视变换
- **DLT**：Direct Linear Transform，从 4 对点对应求单应矩阵的线性方法
- **Otsu**：图像二值化的自动阈值法
- **牛眼**：小程序码定位点的俗称
- **装饰环**：抖音码中不参与编码的环（环 1、环 3）
