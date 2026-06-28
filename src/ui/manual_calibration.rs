use crate::app::QRacerApp;
use crate::code_kind::CodeKind;
use crate::codec::data_matrix_grid::{DATA_MATRIX_SYMBOLS, DataMatrixSymbol};
use crate::pipeline::perspective::homography_from_4pts;
use eframe::egui;
use nalgebra::Matrix3;

const CANVAS_PADDING_RATIO: f32 = 0.08;
const HANDLE_RADIUS: f32 = 6.0;
const HANDLE_HIT_RADIUS: f32 = 16.0;
const EDGE_HANDLE_HALF_SIZE: f32 = 4.0;
const EDGE_HIT_RADIUS: f32 = 10.0;
const ROTATE_HANDLE_RADIUS: f32 = 7.0;
const ROTATE_HANDLE_HIT_RADIUS: f32 = 18.0;
const ROTATE_HANDLE_OFFSET: f32 = 22.0;
const ROTATION_STEP_DEGREES: f32 = 1.0;
const SCROLL_ZOOM_FACTOR_PER_POINT: f32 = 0.00035;
const IMAGE_MESH_STEPS: usize = 18;
const UNDO_LIMIT: usize = 80;
const GRID_REFERENCE_INSET_RATIO: f32 = 0.10;
const WX_MARGIN: f32 = 0.23;
const WX_LOCATOR_RADIUS_RATIO: f32 = 0.0786;
const WX_BADGE_OFFSET_PER_LOCATOR_LEG: f32 = 0.011;
const WX_BADGE_RADIUS_PER_LOCATOR_LEG: f32 = 0.156;
const DY_MARGIN: f32 = 0.23;
const DY_NO_BORDER_LOCATOR_DISTANCE: f32 = 240.529_45;
const DY_NO_BORDER_LOCATOR_RADII: [f32; 3] = [8.13, 18.43, 29.01];
const DY_NO_BORDER_RINGS: [(f32, bool); 6] = [
    (228.66, true),
    (207.98, false),
    (188.59, true),
    (171.71, false),
    (153.74, false),
    (133.24, false),
];
const DY_NO_BORDER_LAYOUT_CENTER: (f32, f32) = (304.32, 307.63);
const DY_NO_BORDER_LAYOUT_BADGE_CENTER: (f32, f32) = (483.49, 128.31);
const DY_NO_BORDER_LAYOUT_BADGE_RADIUS: f32 = 57.3;
const DY_BLACK_BORDER_LOCATOR_DISTANCE: f32 = 261.452;
const DY_BLACK_BORDER_LOCATOR_RADII: [f32; 4] = [8.05, 18.24, 28.71, 36.85];
const DY_BLACK_BORDER_RINGS: [(f32, f32); 5] = [
    (218.42, 231.42),
    (181.84, 190.84),
    (160.87, 169.86),
    (140.20, 149.20),
    (119.20, 128.20),
];
const DY_BLACK_BORDER_OUTER_FRAME: (f32, f32) = (261.10, 283.47);
const DY_BLACK_BORDER_LAYOUT_CENTER: (f32, f32) = (371.02, 371.02);
const DY_BLACK_BORDER_LAYOUT_BADGE_CENTER: (f32, f32) = (564.67, 176.84);
const DY_BLACK_BORDER_LAYOUT_BADGE_RADII: (f32, f32) = (73.04, 85.35);

#[derive(Debug)]
pub struct ManualCalibrationState {
    pub open: bool,
    pub kind: CodeKind,
    qr_version: u8,
    data_matrix_symbol: DataMatrixSymbol,
    douyin_has_border: bool,
    message: Option<ManualCalibrationMessage>,
    corners: [egui::Pos2; 4],
    drag: Option<DragTarget>,
    last_pointer: Option<egui::Pos2>,
    drag_undo_pushed: bool,
    scroll_undo_pushed: bool,
    image_size: Option<(u32, u32)>,
    undo_stack: Vec<[egui::Pos2; 4]>,
}

#[derive(Debug, Clone)]
pub struct ManualCalibrationMessage {
    kind: ManualCalibrationMessageKind,
    text: String,
}

#[derive(Debug, Clone, Copy)]
pub enum ManualCalibrationMessageKind {
    Info,
    Error,
}

#[derive(Debug, Clone, Copy)]
enum DragTarget {
    Corner(usize),
    Edge(ResizeEdge),
    Move,
    Rotate,
}

#[derive(Debug, Clone, Copy)]
enum ResizeEdge {
    Top,
    Right,
    Bottom,
    Left,
}

impl ManualCalibrationState {
    pub fn new() -> Self {
        Self {
            open: false,
            kind: CodeKind::Douyin,
            qr_version: 1,
            data_matrix_symbol: DATA_MATRIX_SYMBOLS[0],
            douyin_has_border: false,
            message: None,
            corners: default_corners((1, 1)),
            drag: None,
            last_pointer: None,
            drag_undo_pushed: false,
            scroll_undo_pushed: false,
            image_size: None,
            undo_stack: Vec::new(),
        }
    }

    pub fn open_for(&mut self, kind: CodeKind, image_size: (u32, u32)) {
        self.open = true;
        self.kind = normalize_kind(kind);
        self.reset(image_size);
    }

    pub fn set_kind(&mut self, kind: CodeKind) {
        self.kind = normalize_kind(kind);
    }

    pub fn set_qr_version_hint(&mut self, version: Option<u8>) {
        if let Some(version) = version.filter(|version| (1..=40).contains(version)) {
            self.qr_version = version;
        }
    }

    pub fn set_data_matrix_symbol_hint(&mut self, symbol: Option<DataMatrixSymbol>) {
        if let Some(symbol) = symbol.filter(|symbol| DATA_MATRIX_SYMBOLS.contains(symbol)) {
            self.data_matrix_symbol = symbol;
        }
    }

    pub fn set_douyin_border_hint(&mut self, has_border: Option<bool>) {
        if let Some(has_border) = has_border {
            self.douyin_has_border = has_border;
        }
    }

    pub fn qr_version(&self) -> u8 {
        self.qr_version
    }

    pub fn data_matrix_symbol(&self) -> DataMatrixSymbol {
        self.data_matrix_symbol
    }

    pub fn douyin_has_border(&self) -> bool {
        self.douyin_has_border
    }

    pub fn close_for_new_image(&mut self) {
        self.open = false;
        self.message = None;
        self.drag = None;
        self.last_pointer = None;
        self.drag_undo_pushed = false;
        self.scroll_undo_pushed = false;
        self.image_size = None;
        self.undo_stack.clear();
    }

    pub fn set_message(&mut self, kind: ManualCalibrationMessageKind, text: impl Into<String>) {
        self.message = Some(ManualCalibrationMessage {
            kind,
            text: text.into(),
        });
    }

    pub fn clear_message(&mut self) {
        self.message = None;
    }

    pub fn output_corners(&self, target_size: u32) -> [(f64, f64); 4] {
        self.output_corners_for_size(target_size, target_size)
    }

    pub fn output_corners_for_size(
        &self,
        target_width: u32,
        target_height: u32,
    ) -> [(f64, f64); 4] {
        self.output_corners_for_size_with_inset(target_width, target_height, 0.0, 0.0)
    }

    pub fn output_grid_corners_for_size(
        &self,
        target_width: u32,
        target_height: u32,
    ) -> [(f64, f64); 4] {
        self.output_corners_for_size_with_inset(
            target_width,
            target_height,
            GRID_REFERENCE_INSET_RATIO,
            GRID_REFERENCE_INSET_RATIO,
        )
    }

    fn output_corners_for_size_with_inset(
        &self,
        target_width: u32,
        target_height: u32,
        inset_x: f32,
        inset_y: f32,
    ) -> [(f64, f64); 4] {
        let max_x = target_width.saturating_sub(1) as f64;
        let max_y = target_height.saturating_sub(1) as f64;
        let scale_x = (1.0 - inset_x * 2.0).max(0.05);
        let scale_y = (1.0 - inset_y * 2.0).max(0.05);
        self.corners.map(|corner| {
            (
                f64::from((corner.x - inset_x) / scale_x) * max_x,
                f64::from((corner.y - inset_y) / scale_y) * max_y,
            )
        })
    }

    fn reset(&mut self, image_size: (u32, u32)) {
        self.corners = default_corners(image_size);
        self.drag = None;
        self.last_pointer = None;
        self.drag_undo_pushed = false;
        self.scroll_undo_pushed = false;
        self.image_size = Some(image_size);
        self.undo_stack.clear();
    }

    fn reset_with_undo(&mut self, image_size: (u32, u32)) {
        let corners = default_corners(image_size);
        if !same_corners(&self.corners, &corners) {
            self.push_undo();
            self.corners = corners;
        }
        self.drag = None;
        self.last_pointer = None;
        self.drag_undo_pushed = false;
        self.scroll_undo_pushed = false;
        self.image_size = Some(image_size);
    }

    fn can_undo(&self) -> bool {
        !self.undo_stack.is_empty()
    }

    fn undo(&mut self) -> bool {
        let Some(corners) = self.undo_stack.pop() else {
            return false;
        };
        self.corners = corners;
        self.drag = None;
        self.last_pointer = None;
        self.drag_undo_pushed = false;
        self.scroll_undo_pushed = false;
        true
    }

    fn rotate_degrees_with_undo(&mut self, degrees: f32) {
        if degrees.abs() <= f32::EPSILON {
            return;
        }
        self.push_undo();
        rotate_about(self, quad_center(self.corners), degrees.to_radians());
    }

    fn push_undo(&mut self) {
        if self
            .undo_stack
            .last()
            .is_some_and(|corners| same_corners(corners, &self.corners))
        {
            return;
        }
        if self.undo_stack.len() >= UNDO_LIMIT {
            self.undo_stack.remove(0);
        }
        self.undo_stack.push(self.corners);
    }
}

pub fn show(ctx: &egui::Context, app: &mut QRacerApp) {
    if !app.manual_calibration.open {
        return;
    }

    let Some((texture, image_size)) = app.original.as_mut().map(|image| {
        let image_size = (image.source.width(), image.source.height());
        (image.texture(ctx).clone(), image_size)
    }) else {
        app.manual_calibration.close_for_new_image();
        app.status = String::from("没有可用于手动校准的原图");
        return;
    };

    if app.manual_calibration.image_size != Some(image_size) {
        app.manual_calibration.reset(image_size);
    }

    let keyboard_undo = ctx.input_mut(|input| {
        input.consume_shortcut(&egui::KeyboardShortcut::new(
            egui::Modifiers::COMMAND,
            egui::Key::Z,
        ))
    });
    let mut open = app.manual_calibration.open;
    let mut reset = false;
    let mut undo = false;
    let mut rotate_degrees = None;
    let mut apply = false;
    let mut close_requested = false;
    let mut selected_kind = None;
    egui::Window::new("手动校准")
        .open(&mut open)
        .resizable(true)
        .default_size(egui::vec2(960.0, 560.0))
        .min_width(520.0)
        .show(ctx, |ui| {
            ui.horizontal_wrapped(|ui| {
                ui.label("参考：");
                egui::ComboBox::from_id_salt("manual-calibration-kind")
                    .selected_text(app.manual_calibration.kind.label())
                    .show_ui(ui, |ui| {
                        for kind in CodeKind::MANUAL_CALIBRATION {
                            if ui
                                .selectable_label(app.manual_calibration.kind == kind, kind.label())
                                .clicked()
                            {
                                selected_kind = Some(kind);
                                ui.close();
                            }
                        }
                    });
                match app.manual_calibration.kind {
                    CodeKind::Qr => {
                        ui.label("版本:");
                        egui::ComboBox::from_id_salt("manual-calibration-qr-version")
                            .selected_text(qr_version_label(app.manual_calibration.qr_version))
                            .show_ui(ui, |ui| {
                                for version in 1..=40 {
                                    if ui
                                        .selectable_label(
                                            app.manual_calibration.qr_version == version,
                                            qr_version_label(version),
                                        )
                                        .clicked()
                                    {
                                        app.manual_calibration.qr_version = version;
                                        ui.close();
                                    }
                                }
                            });
                    }
                    CodeKind::DataMatrix => {
                        ui.label("样式:");
                        egui::ComboBox::from_id_salt("manual-calibration-data-matrix-symbol")
                            .selected_text(data_matrix_symbol_label(
                                app.manual_calibration.data_matrix_symbol,
                            ))
                            .show_ui(ui, |ui| {
                                for &symbol in DATA_MATRIX_SYMBOLS {
                                    if ui
                                        .selectable_label(
                                            app.manual_calibration.data_matrix_symbol == symbol,
                                            data_matrix_symbol_label(symbol),
                                        )
                                        .clicked()
                                    {
                                        app.manual_calibration.data_matrix_symbol = symbol;
                                        ui.close();
                                    }
                                }
                            });
                    }
                    CodeKind::Douyin => {
                        ui.label("外观:");
                        if ui
                            .selectable_label(!app.manual_calibration.douyin_has_border, "无框版")
                            .clicked()
                        {
                            app.manual_calibration.douyin_has_border = false;
                        }
                        if ui
                            .selectable_label(app.manual_calibration.douyin_has_border, "黑框版")
                            .clicked()
                        {
                            app.manual_calibration.douyin_has_border = true;
                        }
                    }
                    CodeKind::WxMiniprogram | CodeKind::Unknown => {}
                }
                ui.separator();
                if ui
                    .add_enabled(app.manual_calibration.can_undo(), egui::Button::new("撤销"))
                    .clicked()
                {
                    undo = true;
                }
                if ui.button("逆时针 1°").clicked() {
                    rotate_degrees = Some(-ROTATION_STEP_DEGREES);
                }
                if ui.button("顺时针 1°").clicked() {
                    rotate_degrees = Some(ROTATION_STEP_DEGREES);
                }
                if ui.button("重置").clicked() {
                    reset = true;
                }
                if ui.button("应用校准").clicked() {
                    apply = true;
                }
                if ui.button("取消").clicked() {
                    close_requested = true;
                }
            });
            ui.separator();
            if let Some(message) = app.manual_calibration.message.as_ref() {
                let color = match message.kind {
                    ManualCalibrationMessageKind::Info => egui::Color32::from_rgb(45, 125, 210),
                    ManualCalibrationMessageKind::Error => egui::Color32::from_rgb(190, 55, 45),
                };
                ui.colored_label(color, &message.text);
                ui.separator();
            }
            ui.horizontal_wrapped(|ui| {
                ui.label("操作：拖动图片可整体移动；按住 Shift 等比例缩放；按住 Ctrl 拖动四角可自由变换；拖动画面上方圆点可旋转；滚轮可缩放；Ctrl+Z 可撤销上一步。");
            });
            ui.separator();

            calibration_canvas(ui, &texture, &mut app.manual_calibration);
        });

    if close_requested {
        open = false;
    }
    app.manual_calibration.open = open;
    if let Some(kind) = selected_kind {
        app.manual_calibration.set_kind(kind);
        app.manual_calibration.set_qr_version_hint(
            app.qr_version
                .or_else(|| app.last_decoded.as_ref().map(|decoded| decoded.version)),
        );
        app.manual_calibration.set_data_matrix_symbol_hint(
            app.last_data_matrix_grid.as_ref().map(|grid| grid.symbol),
        );
        app.manual_calibration
            .set_douyin_border_hint(app.last_dy_grid.as_ref().map(|grid| grid.has_border));
    }
    if keyboard_undo || undo {
        app.manual_calibration.undo();
    }
    if let Some(degrees) = rotate_degrees {
        app.manual_calibration.rotate_degrees_with_undo(degrees);
    }
    if reset {
        app.manual_calibration.reset_with_undo(image_size);
    }
    if apply {
        app.apply_manual_calibration();
    }
}

fn calibration_canvas(
    ui: &mut egui::Ui,
    texture: &egui::TextureHandle,
    state: &mut ManualCalibrationState,
) {
    let available = ui.available_size();
    let side = available.x.min(available.y.max(420.0)).max(360.0);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::click_and_drag());
    let reference_rect = fit_reference_rect(
        rect.shrink(side * CANVAS_PADDING_RATIO),
        reference_aspect_ratio(state),
    );
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(32, 34, 36));
    painter.rect_filled(reference_rect, 0.0, egui::Color32::from_rgb(248, 248, 246));

    draw_transformed_image(&painter, texture, state, reference_rect);
    draw_reference(&painter, reference_rect, state);
    draw_transform_handles(&painter, reference_rect, state);
    handle_canvas_input(ui, &response, reference_rect, state);
}

fn draw_transformed_image(
    painter: &egui::Painter,
    texture: &egui::TextureHandle,
    state: &ManualCalibrationState,
    reference_rect: egui::Rect,
) {
    let dst = state
        .corners
        .map(|corner| normalized_to_screen(reference_rect, corner));
    let src = [(0.0, 0.0), (1.0, 0.0), (0.0, 1.0), (1.0, 1.0)];
    let dst_h = [
        (f64::from(dst[0].x), f64::from(dst[0].y)),
        (f64::from(dst[1].x), f64::from(dst[1].y)),
        (f64::from(dst[2].x), f64::from(dst[2].y)),
        (f64::from(dst[3].x), f64::from(dst[3].y)),
    ];
    let h = homography_from_4pts(&src, &dst_h);
    let mut mesh = egui::Mesh::with_texture(texture.id());
    let color = egui::Color32::from_white_alpha(210);

    for y in 0..=IMAGE_MESH_STEPS {
        let v = y as f32 / IMAGE_MESH_STEPS as f32;
        for x in 0..=IMAGE_MESH_STEPS {
            let u = x as f32 / IMAGE_MESH_STEPS as f32;
            mesh.vertices.push(egui::epaint::Vertex {
                pos: project_unit_point(&h, u, v),
                uv: egui::pos2(u, v),
                color,
            });
        }
    }

    let row = IMAGE_MESH_STEPS + 1;
    for y in 0..IMAGE_MESH_STEPS {
        for x in 0..IMAGE_MESH_STEPS {
            let a = (y * row + x) as u32;
            let b = a + 1;
            let c = ((y + 1) * row + x) as u32;
            let d = c + 1;
            mesh.indices.extend_from_slice(&[a, b, c, c, b, d]);
        }
    }

    painter.add(egui::Shape::mesh(mesh));
}

fn draw_reference(painter: &egui::Painter, rect: egui::Rect, state: &ManualCalibrationState) {
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.5, egui::Color32::from_rgb(40, 170, 210)),
        egui::StrokeKind::Inside,
    );

    match state.kind {
        CodeKind::Qr => draw_qr_reference(painter, grid_reference_rect(rect), state.qr_version),
        CodeKind::DataMatrix => {
            draw_data_matrix_reference(painter, grid_reference_rect(rect), state.data_matrix_symbol)
        }
        CodeKind::WxMiniprogram => draw_wx_reference(painter, rect),
        CodeKind::Douyin => draw_dy_reference(painter, rect, state.douyin_has_border),
        CodeKind::Unknown => {}
    }
}

fn draw_qr_reference(painter: &egui::Painter, rect: egui::Rect, version: u8) {
    let modules = qr_modules_for_version(version);
    draw_sparse_grid(
        painter,
        rect,
        modules,
        modules,
        sparse_grid_step(modules),
        egui::Stroke::new(
            0.55,
            egui::Color32::from_rgba_unmultiplied(40, 170, 210, 58),
        ),
    );

    let finder_stroke = egui::Stroke::new(1.6, egui::Color32::from_rgb(25, 125, 210));
    for (x, y) in [(0, 0), (modules - 7, 0), (0, modules - 7)] {
        draw_module_rect_stroke(painter, rect, modules, modules, x, y, 7, 7, finder_stroke);
        draw_module_rect_stroke(
            painter,
            rect,
            modules,
            modules,
            x + 1,
            y + 1,
            5,
            5,
            finder_stroke,
        );
        draw_module_rect_stroke(
            painter,
            rect,
            modules,
            modules,
            x + 2,
            y + 2,
            3,
            3,
            finder_stroke,
        );
        draw_module_rect_stroke(
            painter,
            rect,
            modules,
            modules,
            x.saturating_sub(1),
            y.saturating_sub(1),
            if x == 0 { 8 } else { 9 }.min(modules - x.saturating_sub(1)),
            if y == 0 { 8 } else { 9 }.min(modules - y.saturating_sub(1)),
            egui::Stroke::new(
                0.8,
                egui::Color32::from_rgba_unmultiplied(245, 245, 245, 150),
            ),
        );
    }

    let timing_color = egui::Color32::from_rgba_unmultiplied(230, 145, 35, 170);
    for i in 8..modules.saturating_sub(8) {
        if i % 2 == 0 {
            draw_module_marker(painter, rect, modules, modules, i, 6, timing_color);
            draw_module_marker(painter, rect, modules, modules, 6, i, timing_color);
        }
    }

    let align_stroke = egui::Stroke::new(1.1, egui::Color32::from_rgb(40, 160, 95));
    for cy in qr_alignment_pattern_positions(version, modules) {
        for cx in qr_alignment_pattern_positions(version, modules) {
            if qr_alignment_overlaps_finder(cx, cy, modules) {
                continue;
            }
            draw_module_rect_stroke(
                painter,
                rect,
                modules,
                modules,
                cx - 2,
                cy - 2,
                5,
                5,
                align_stroke,
            );
            draw_module_rect_stroke(painter, rect, modules, modules, cx, cy, 1, 1, align_stroke);
        }
    }
}

fn draw_data_matrix_reference(painter: &egui::Painter, rect: egui::Rect, symbol: DataMatrixSymbol) {
    let cols = symbol.cols;
    let rows = symbol.rows;
    draw_sparse_grid(
        painter,
        rect,
        cols,
        rows,
        sparse_grid_step(cols.max(rows)),
        egui::Stroke::new(
            0.55,
            egui::Color32::from_rgba_unmultiplied(40, 170, 210, 52),
        ),
    );

    let region_w = symbol.region_cols + 2;
    let region_h = symbol.region_rows + 2;
    let boundary_stroke = egui::Stroke::new(
        1.0,
        egui::Color32::from_rgba_unmultiplied(30, 135, 205, 112),
    );
    for x in (region_w..cols).step_by(region_w) {
        draw_grid_line_x(painter, rect, cols, x, boundary_stroke);
    }
    for y in (region_h..rows).step_by(region_h) {
        draw_grid_line_y(painter, rect, rows, y, boundary_stroke);
    }

    let solid_color = egui::Color32::from_rgba_unmultiplied(30, 120, 70, 185);
    let timing_color = egui::Color32::from_rgba_unmultiplied(225, 135, 30, 180);
    for y0 in (0..rows).step_by(region_h) {
        for x0 in (0..cols).step_by(region_w) {
            let y_end = (y0 + region_h).min(rows);
            let x_end = (x0 + region_w).min(cols);
            for y in y0..y_end {
                draw_module_marker(painter, rect, cols, rows, x0, y, solid_color);
                if (y - y0) % 2 == 1 {
                    draw_module_marker(painter, rect, cols, rows, x_end - 1, y, timing_color);
                }
            }
            for x in x0..x_end {
                draw_module_marker(painter, rect, cols, rows, x, y_end - 1, solid_color);
                if (x - x0) % 2 == 0 {
                    draw_module_marker(painter, rect, cols, rows, x, y0, timing_color);
                }
            }
        }
    }
}

fn draw_wx_reference(painter: &egui::Painter, rect: egui::Rect) {
    let margin = WX_MARGIN;
    let far = 1.0 - margin;
    let leg = far - margin;
    let center = egui::pos2(0.5, 0.5);
    let locator_radius = leg * WX_LOCATOR_RADIUS_RATIO;
    let locator_distance = ((0.5_f32 - margin).powi(2) * 2.0).sqrt();
    let r_max = locator_distance + locator_radius * 1.41;
    let r_min = r_max * 0.50;

    for radius in [r_min, (r_min + r_max) * 0.5, r_max] {
        draw_circle_normalized(
            painter,
            rect,
            center,
            radius,
            egui::Stroke::new(1.2, egui::Color32::from_rgb(45, 170, 110)),
        );
    }

    for point in [
        egui::pos2(margin, margin),
        egui::pos2(far, margin),
        egui::pos2(margin, far),
    ] {
        draw_bullseye(
            painter,
            rect,
            point,
            &[locator_radius, locator_radius * 0.62, locator_radius * 0.18],
            egui::Color32::from_rgb(20, 120, 80),
        );
    }

    let badge_center = egui::pos2(
        far + leg * WX_BADGE_OFFSET_PER_LOCATOR_LEG,
        far + leg * WX_BADGE_OFFSET_PER_LOCATOR_LEG,
    );
    let badge_radius = leg * WX_BADGE_RADIUS_PER_LOCATOR_LEG;
    draw_circle_normalized(
        painter,
        rect,
        badge_center,
        badge_radius,
        egui::Stroke::new(1.4, egui::Color32::from_rgb(170, 60, 170)),
    );
}

fn draw_dy_reference(painter: &egui::Painter, rect: egui::Rect, has_border: bool) {
    if has_border {
        draw_dy_black_border_reference(painter, rect);
    } else {
        draw_dy_no_border_reference(painter, rect);
    }
}

fn draw_dy_no_border_reference(painter: &egui::Painter, rect: egui::Rect) {
    let loc_dist = dy_locator_distance_norm();
    let center = egui::pos2(0.5, 0.5);
    for (radius, decorative) in DY_NO_BORDER_RINGS {
        let color = if decorative {
            egui::Color32::from_rgb(220, 140, 35)
        } else {
            egui::Color32::from_rgb(35, 155, 215)
        };
        draw_circle_normalized(
            painter,
            rect,
            center,
            radius / DY_NO_BORDER_LOCATOR_DISTANCE * loc_dist,
            egui::Stroke::new(1.2, color),
        );
    }

    let badge_offset = egui::vec2(
        (DY_NO_BORDER_LAYOUT_BADGE_CENTER.0 - DY_NO_BORDER_LAYOUT_CENTER.0)
            / DY_NO_BORDER_LOCATOR_DISTANCE
            * loc_dist,
        (DY_NO_BORDER_LAYOUT_BADGE_CENTER.1 - DY_NO_BORDER_LAYOUT_CENTER.1)
            / DY_NO_BORDER_LOCATOR_DISTANCE
            * loc_dist,
    );
    draw_circle_normalized(
        painter,
        rect,
        center + badge_offset,
        DY_NO_BORDER_LAYOUT_BADGE_RADIUS / DY_NO_BORDER_LOCATOR_DISTANCE * loc_dist,
        egui::Stroke::new(1.2, egui::Color32::from_rgb(170, 60, 170)),
    );

    draw_dy_locators(
        painter,
        rect,
        &DY_NO_BORDER_LOCATOR_RADII.map(|radius| radius / DY_NO_BORDER_LOCATOR_DISTANCE * loc_dist),
    );
}

fn draw_dy_black_border_reference(painter: &egui::Painter, rect: egui::Rect) {
    let loc_dist = dy_locator_distance_norm();
    let center = egui::pos2(0.5, 0.5);
    for (inner, outer) in DY_BLACK_BORDER_RINGS {
        draw_circle_normalized(
            painter,
            rect,
            center,
            inner / DY_BLACK_BORDER_LOCATOR_DISTANCE * loc_dist,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(35, 155, 215)),
        );
        draw_circle_normalized(
            painter,
            rect,
            center,
            outer / DY_BLACK_BORDER_LOCATOR_DISTANCE * loc_dist,
            egui::Stroke::new(1.0, egui::Color32::from_rgb(35, 155, 215)),
        );
    }
    for radius in [DY_BLACK_BORDER_OUTER_FRAME.0, DY_BLACK_BORDER_OUTER_FRAME.1] {
        draw_circle_normalized(
            painter,
            rect,
            center,
            radius / DY_BLACK_BORDER_LOCATOR_DISTANCE * loc_dist,
            egui::Stroke::new(1.3, egui::Color32::from_rgb(220, 140, 35)),
        );
    }

    let badge_offset = egui::vec2(
        (DY_BLACK_BORDER_LAYOUT_BADGE_CENTER.0 - DY_BLACK_BORDER_LAYOUT_CENTER.0)
            / DY_BLACK_BORDER_LOCATOR_DISTANCE
            * loc_dist,
        (DY_BLACK_BORDER_LAYOUT_BADGE_CENTER.1 - DY_BLACK_BORDER_LAYOUT_CENTER.1)
            / DY_BLACK_BORDER_LOCATOR_DISTANCE
            * loc_dist,
    );
    draw_badge_reference(
        painter,
        rect,
        center + badge_offset,
        DY_BLACK_BORDER_LAYOUT_BADGE_RADII.1 / DY_BLACK_BORDER_LOCATOR_DISTANCE * loc_dist,
        DY_BLACK_BORDER_LAYOUT_BADGE_RADII.0 / DY_BLACK_BORDER_LOCATOR_DISTANCE * loc_dist,
    );

    draw_dy_locators(
        painter,
        rect,
        &DY_BLACK_BORDER_LOCATOR_RADII
            .map(|radius| radius / DY_BLACK_BORDER_LOCATOR_DISTANCE * loc_dist),
    );
}

fn draw_dy_locators(painter: &egui::Painter, rect: egui::Rect, radii: &[f32]) {
    let margin = DY_MARGIN;
    let far = 1.0 - margin;
    for point in [
        egui::pos2(margin, margin),
        egui::pos2(margin, far),
        egui::pos2(far, far),
    ] {
        draw_bullseye(
            painter,
            rect,
            point,
            radii,
            egui::Color32::from_rgb(30, 80, 180),
        );
    }
}

fn draw_bullseye(
    painter: &egui::Painter,
    rect: egui::Rect,
    center: egui::Pos2,
    radii: &[f32],
    color: egui::Color32,
) {
    for (idx, radius) in radii.iter().copied().enumerate().rev() {
        let alpha = if idx % 2 == 0 { 185 } else { 90 };
        draw_circle_normalized(
            painter,
            rect,
            center,
            radius,
            egui::Stroke::new(1.2, color.gamma_multiply(alpha as f32 / 255.0)),
        );
    }
}

fn draw_badge_reference(
    painter: &egui::Painter,
    rect: egui::Rect,
    center: egui::Pos2,
    outer_radius: f32,
    inner_radius: f32,
) {
    let color = egui::Color32::from_rgb(170, 60, 170);
    draw_circle_normalized(
        painter,
        rect,
        center,
        outer_radius,
        egui::Stroke::new(1.4, color),
    );
    draw_circle_normalized(
        painter,
        rect,
        center,
        inner_radius,
        egui::Stroke::new(1.0, color.gamma_multiply(0.58)),
    );
}

fn draw_circle_normalized(
    painter: &egui::Painter,
    rect: egui::Rect,
    center: egui::Pos2,
    radius: f32,
    stroke: egui::Stroke,
) {
    painter.circle_stroke(
        normalized_to_screen(rect, center),
        radius * rect.width().min(rect.height()),
        stroke,
    );
}

fn draw_sparse_grid(
    painter: &egui::Painter,
    rect: egui::Rect,
    cols: usize,
    rows: usize,
    step: usize,
    stroke: egui::Stroke,
) {
    if cols == 0 || rows == 0 || step == 0 {
        return;
    }
    for x in (step..cols).step_by(step) {
        draw_grid_line_x(painter, rect, cols, x, stroke);
    }
    for y in (step..rows).step_by(step) {
        draw_grid_line_y(painter, rect, rows, y, stroke);
    }
}

fn draw_grid_line_x(
    painter: &egui::Painter,
    rect: egui::Rect,
    cols: usize,
    x: usize,
    stroke: egui::Stroke,
) {
    let sx = rect.left() + rect.width() * x as f32 / cols.max(1) as f32;
    painter.line_segment(
        [egui::pos2(sx, rect.top()), egui::pos2(sx, rect.bottom())],
        stroke,
    );
}

fn draw_grid_line_y(
    painter: &egui::Painter,
    rect: egui::Rect,
    rows: usize,
    y: usize,
    stroke: egui::Stroke,
) {
    let sy = rect.top() + rect.height() * y as f32 / rows.max(1) as f32;
    painter.line_segment(
        [egui::pos2(rect.left(), sy), egui::pos2(rect.right(), sy)],
        stroke,
    );
}

fn draw_module_marker(
    painter: &egui::Painter,
    rect: egui::Rect,
    cols: usize,
    rows: usize,
    x: usize,
    y: usize,
    color: egui::Color32,
) {
    if x >= cols || y >= rows {
        return;
    }
    let module = module_rect(rect, cols, rows, x, y, 1, 1);
    let inset = module.width().min(module.height()) * 0.18;
    painter.rect_filled(module.shrink(inset), 0.0, color);
}

fn draw_module_rect_stroke(
    painter: &egui::Painter,
    rect: egui::Rect,
    cols: usize,
    rows: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
    stroke: egui::Stroke,
) {
    if x >= cols || y >= rows || w == 0 || h == 0 {
        return;
    }
    let module = module_rect(rect, cols, rows, x, y, w.min(cols - x), h.min(rows - y));
    painter.rect_stroke(module, 0.0, stroke, egui::StrokeKind::Inside);
}

fn module_rect(
    rect: egui::Rect,
    cols: usize,
    rows: usize,
    x: usize,
    y: usize,
    w: usize,
    h: usize,
) -> egui::Rect {
    let cols = cols.max(1) as f32;
    let rows = rows.max(1) as f32;
    egui::Rect::from_min_max(
        egui::pos2(
            rect.left() + rect.width() * x as f32 / cols,
            rect.top() + rect.height() * y as f32 / rows,
        ),
        egui::pos2(
            rect.left() + rect.width() * (x + w) as f32 / cols,
            rect.top() + rect.height() * (y + h) as f32 / rows,
        ),
    )
}

fn sparse_grid_step(modules: usize) -> usize {
    if modules <= 32 {
        2
    } else if modules <= 80 {
        4
    } else {
        8
    }
}

fn qr_modules_for_version(version: u8) -> usize {
    (version.clamp(1, 40) as usize - 1) * 4 + 21
}

fn qr_alignment_pattern_positions(version: u8, modules: usize) -> Vec<usize> {
    if version <= 1 {
        return Vec::new();
    }
    let count = version as usize / 7 + 2;
    let step = if version == 32 {
        26
    } else {
        ((version as usize * 4 + count * 2 + 1) / (count * 2 - 2)) * 2
    };
    let mut positions = vec![6; count];
    let mut pos = modules - 7;
    for index in (1..count).rev() {
        positions[index] = pos;
        pos = pos.saturating_sub(step);
    }
    positions
}

fn qr_alignment_overlaps_finder(cx: usize, cy: usize, modules: usize) -> bool {
    (cx == 6 && (cy == 6 || cy == modules - 7)) || (cy == 6 && cx == modules - 7)
}

fn draw_transform_handles(
    painter: &egui::Painter,
    reference_rect: egui::Rect,
    state: &ManualCalibrationState,
) {
    let points = state
        .corners
        .map(|corner| normalized_to_screen(reference_rect, corner));
    let stroke = egui::Stroke::new(1.5, egui::Color32::from_rgb(250, 210, 70));
    let rotate_handle = rotation_handle_screen(reference_rect, state);
    let top_mid = midpoint(points[0], points[1]);

    painter.line_segment([points[0], points[1]], stroke);
    painter.line_segment([points[1], points[3]], stroke);
    painter.line_segment([points[3], points[2]], stroke);
    painter.line_segment([points[2], points[0]], stroke);
    painter.line_segment([top_mid, rotate_handle], stroke);
    for point in edge_midpoints(points) {
        let rect = egui::Rect::from_center_size(
            point,
            egui::vec2(EDGE_HANDLE_HALF_SIZE * 2.0, EDGE_HANDLE_HALF_SIZE * 2.0),
        );
        painter.rect_filled(rect, 1.5, egui::Color32::from_rgb(250, 210, 70));
        painter.rect_stroke(
            rect,
            1.5,
            egui::Stroke::new(1.0, egui::Color32::BLACK),
            egui::StrokeKind::Inside,
        );
    }
    painter.circle_filled(
        rotate_handle,
        ROTATE_HANDLE_RADIUS,
        egui::Color32::from_rgb(250, 210, 70),
    );
    painter.circle_stroke(
        rotate_handle,
        ROTATE_HANDLE_RADIUS,
        egui::Stroke::new(1.0, egui::Color32::BLACK),
    );
    for point in points {
        painter.circle_filled(point, HANDLE_RADIUS, egui::Color32::from_rgb(250, 210, 70));
        painter.circle_stroke(
            point,
            HANDLE_RADIUS,
            egui::Stroke::new(1.0, egui::Color32::BLACK),
        );
    }
}

fn handle_canvas_input(
    ui: &egui::Ui,
    response: &egui::Response,
    reference_rect: egui::Rect,
    state: &mut ManualCalibrationState,
) {
    if response.hovered() {
        let scroll = ui.input(|input| input.smooth_scroll_delta.y);
        if scroll.abs() > f32::EPSILON {
            let factor = (1.0_f32 + scroll * SCROLL_ZOOM_FACTOR_PER_POINT).clamp(0.96, 1.04);
            let pivot = response
                .hover_pos()
                .map(|pos| screen_to_normalized(reference_rect, pos))
                .unwrap_or(egui::pos2(0.5, 0.5));
            if !state.scroll_undo_pushed {
                state.push_undo();
                state.scroll_undo_pushed = true;
            }
            scale_about(state, pivot, factor);
        } else {
            state.scroll_undo_pushed = false;
        }
    } else {
        state.scroll_undo_pushed = false;
    }

    if response.drag_started() {
        if let Some(pointer) = response.interact_pointer_pos() {
            state.drag = hit_test(pointer, reference_rect, state);
            state.last_pointer = Some(pointer);
            state.drag_undo_pushed = false;
        }
    }

    if response.dragged() {
        if let (Some(target), Some(last), Some(pointer)) = (
            state.drag,
            state.last_pointer,
            response.interact_pointer_pos(),
        ) {
            let delta = egui::vec2(
                (pointer.x - last.x) / reference_rect.width(),
                (pointer.y - last.y) / reference_rect.height(),
            );
            if !state.drag_undo_pushed {
                state.push_undo();
                state.drag_undo_pushed = true;
            }
            let modifiers = ui.input(|input| input.modifiers);
            match target {
                DragTarget::Corner(index) => {
                    drag_corner(state, index, delta, modifiers);
                }
                DragTarget::Edge(edge) => {
                    drag_edge(state, edge, delta, modifiers);
                }
                DragTarget::Move => {
                    for corner in &mut state.corners {
                        *corner += delta;
                    }
                }
                DragTarget::Rotate => {
                    let pivot = quad_center(state.corners);
                    if let Some(angle) = rotation_delta(reference_rect, pivot, last, pointer) {
                        rotate_about(state, pivot, angle);
                    }
                }
            }
            state.last_pointer = Some(pointer);
        }
    }

    if !ui.input(|input| input.pointer.primary_down()) {
        state.drag = None;
        state.last_pointer = None;
        state.drag_undo_pushed = false;
    }
}

fn hit_test(
    pointer: egui::Pos2,
    reference_rect: egui::Rect,
    state: &ManualCalibrationState,
) -> Option<DragTarget> {
    let points = state
        .corners
        .map(|corner| normalized_to_screen(reference_rect, corner));
    for (idx, point) in points.iter().enumerate() {
        if point.distance(pointer) <= HANDLE_HIT_RADIUS {
            return Some(DragTarget::Corner(idx));
        }
    }
    if rotation_handle_screen(reference_rect, state).distance(pointer) <= ROTATE_HANDLE_HIT_RADIUS {
        return Some(DragTarget::Rotate);
    }
    for (edge, a, b) in edge_segments(points) {
        if distance_to_segment(pointer, a, b) <= EDGE_HIT_RADIUS {
            return Some(DragTarget::Edge(edge));
        }
    }
    point_in_quad(pointer, points).then_some(DragTarget::Move)
}

fn drag_corner(
    state: &mut ManualCalibrationState,
    index: usize,
    delta: egui::Vec2,
    modifiers: egui::Modifiers,
) {
    if modifiers.ctrl {
        state.corners[index] += delta;
        return;
    }

    let pivot = state.corners[opposite_corner(index)];
    let dragged = state.corners[index];
    let target = dragged + delta;
    if modifiers.shift {
        let factor = projected_scale_ratio(dragged - pivot, target - pivot);
        scale_about(state, pivot, factor);
    } else {
        let sx = scale_ratio(target.x - pivot.x, dragged.x - pivot.x);
        let sy = scale_ratio(target.y - pivot.y, dragged.y - pivot.y);
        scale_about_axes(state, pivot, sx, sy);
    }
}

fn drag_edge(
    state: &mut ManualCalibrationState,
    edge: ResizeEdge,
    delta: egui::Vec2,
    modifiers: egui::Modifiers,
) {
    let delta = constrained_edge_delta(edge, delta);
    if modifiers.shift {
        let pivot = quad_center(state.corners);
        let edge_center = edge_center(state.corners, edge);
        let factor = projected_scale_ratio(edge_center - pivot, edge_center + delta - pivot);
        scale_about(state, pivot, factor);
        return;
    }

    for index in edge_corner_indices(edge) {
        state.corners[index] += delta;
    }
}

fn constrained_edge_delta(edge: ResizeEdge, delta: egui::Vec2) -> egui::Vec2 {
    match edge {
        ResizeEdge::Top | ResizeEdge::Bottom => egui::vec2(0.0, delta.y),
        ResizeEdge::Right | ResizeEdge::Left => egui::vec2(delta.x, 0.0),
    }
}

fn edge_corner_indices(edge: ResizeEdge) -> [usize; 2] {
    match edge {
        ResizeEdge::Top => [0, 1],
        ResizeEdge::Right => [1, 3],
        ResizeEdge::Bottom => [2, 3],
        ResizeEdge::Left => [0, 2],
    }
}

fn opposite_corner(index: usize) -> usize {
    match index {
        0 => 3,
        1 => 2,
        2 => 1,
        3 => 0,
        _ => 0,
    }
}

fn edge_center(corners: [egui::Pos2; 4], edge: ResizeEdge) -> egui::Pos2 {
    let [a, b] = edge_corner_indices(edge);
    midpoint(corners[a], corners[b])
}

fn projected_scale_ratio(from: egui::Vec2, to: egui::Vec2) -> f32 {
    let len_sq = from.length_sq();
    if len_sq <= 1e-8 {
        return 1.0;
    }
    ((to.x * from.x + to.y * from.y) / len_sq).clamp(0.02, 50.0)
}

fn scale_ratio(to: f32, from: f32) -> f32 {
    if from.abs() <= 1e-6 {
        1.0
    } else {
        (to / from).clamp(0.02, 50.0)
    }
}

fn scale_about(state: &mut ManualCalibrationState, pivot: egui::Pos2, factor: f32) {
    for corner in &mut state.corners {
        let offset = *corner - pivot;
        *corner = pivot + offset * factor;
    }
}

fn scale_about_axes(state: &mut ManualCalibrationState, pivot: egui::Pos2, sx: f32, sy: f32) {
    for corner in &mut state.corners {
        let offset = *corner - pivot;
        *corner = egui::pos2(pivot.x + offset.x * sx, pivot.y + offset.y * sy);
    }
}

fn rotate_about(state: &mut ManualCalibrationState, pivot: egui::Pos2, radians: f32) {
    let (sin, cos) = radians.sin_cos();
    for corner in &mut state.corners {
        let offset = *corner - pivot;
        *corner = egui::pos2(
            pivot.x + offset.x * cos - offset.y * sin,
            pivot.y + offset.x * sin + offset.y * cos,
        );
    }
}

fn rotation_delta(
    reference_rect: egui::Rect,
    pivot: egui::Pos2,
    previous: egui::Pos2,
    current: egui::Pos2,
) -> Option<f32> {
    let previous = screen_to_normalized(reference_rect, previous);
    let current = screen_to_normalized(reference_rect, current);
    Some(normalize_angle(
        angle_from(pivot, current)? - angle_from(pivot, previous)?,
    ))
}

fn angle_from(pivot: egui::Pos2, point: egui::Pos2) -> Option<f32> {
    let offset = point - pivot;
    (offset.length_sq() > f32::EPSILON).then_some(offset.y.atan2(offset.x))
}

fn normalize_angle(radians: f32) -> f32 {
    let tau = std::f32::consts::TAU;
    (radians + std::f32::consts::PI).rem_euclid(tau) - std::f32::consts::PI
}

fn quad_center(corners: [egui::Pos2; 4]) -> egui::Pos2 {
    egui::pos2(
        (corners[0].x + corners[1].x + corners[2].x + corners[3].x) * 0.25,
        (corners[0].y + corners[1].y + corners[2].y + corners[3].y) * 0.25,
    )
}

fn rotation_handle_screen(
    reference_rect: egui::Rect,
    state: &ManualCalibrationState,
) -> egui::Pos2 {
    let points = state
        .corners
        .map(|corner| normalized_to_screen(reference_rect, corner));
    let center = quad_center(points);
    let top_mid = midpoint(points[0], points[1]);
    let offset = top_mid - center;
    let direction = if offset.length_sq() > f32::EPSILON {
        offset.normalized()
    } else {
        egui::vec2(0.0, -1.0)
    };
    top_mid + direction * ROTATE_HANDLE_OFFSET
}

fn midpoint(a: egui::Pos2, b: egui::Pos2) -> egui::Pos2 {
    egui::pos2((a.x + b.x) * 0.5, (a.y + b.y) * 0.5)
}

fn edge_midpoints(points: [egui::Pos2; 4]) -> [egui::Pos2; 4] {
    [
        midpoint(points[0], points[1]),
        midpoint(points[1], points[3]),
        midpoint(points[2], points[3]),
        midpoint(points[0], points[2]),
    ]
}

fn edge_segments(points: [egui::Pos2; 4]) -> [(ResizeEdge, egui::Pos2, egui::Pos2); 4] {
    [
        (ResizeEdge::Top, points[0], points[1]),
        (ResizeEdge::Right, points[1], points[3]),
        (ResizeEdge::Bottom, points[2], points[3]),
        (ResizeEdge::Left, points[0], points[2]),
    ]
}

fn distance_to_segment(point: egui::Pos2, a: egui::Pos2, b: egui::Pos2) -> f32 {
    let ab = b - a;
    let len_sq = ab.length_sq();
    if len_sq <= f32::EPSILON {
        return point.distance(a);
    }
    let ap = point - a;
    let t = ((ap.x * ab.x + ap.y * ab.y) / len_sq).clamp(0.0, 1.0);
    point.distance(a + ab * t)
}

fn same_corners(lhs: &[egui::Pos2; 4], rhs: &[egui::Pos2; 4]) -> bool {
    lhs.iter()
        .zip(rhs)
        .all(|(lhs, rhs)| lhs.distance_sq(*rhs) <= 1e-10)
}

fn point_in_quad(point: egui::Pos2, quad: [egui::Pos2; 4]) -> bool {
    point_in_triangle(point, quad[0], quad[1], quad[2])
        || point_in_triangle(point, quad[2], quad[1], quad[3])
}

fn point_in_triangle(point: egui::Pos2, a: egui::Pos2, b: egui::Pos2, c: egui::Pos2) -> bool {
    let d1 = signed_area(point, a, b);
    let d2 = signed_area(point, b, c);
    let d3 = signed_area(point, c, a);
    let has_neg = d1 < 0.0 || d2 < 0.0 || d3 < 0.0;
    let has_pos = d1 > 0.0 || d2 > 0.0 || d3 > 0.0;
    !(has_neg && has_pos)
}

fn signed_area(p1: egui::Pos2, p2: egui::Pos2, p3: egui::Pos2) -> f32 {
    (p1.x - p3.x) * (p2.y - p3.y) - (p2.x - p3.x) * (p1.y - p3.y)
}

fn normalized_to_screen(rect: egui::Rect, point: egui::Pos2) -> egui::Pos2 {
    egui::pos2(
        rect.left() + point.x * rect.width(),
        rect.top() + point.y * rect.height(),
    )
}

fn screen_to_normalized(rect: egui::Rect, point: egui::Pos2) -> egui::Pos2 {
    egui::pos2(
        (point.x - rect.left()) / rect.width(),
        (point.y - rect.top()) / rect.height(),
    )
}

fn fit_reference_rect(bounds: egui::Rect, aspect: f32) -> egui::Rect {
    let aspect = aspect.max(0.05);
    let bounds_aspect = bounds.width() / bounds.height().max(f32::EPSILON);
    if bounds_aspect > aspect {
        let width = bounds.height() * aspect;
        let left = bounds.center().x - width * 0.5;
        egui::Rect::from_min_size(
            egui::pos2(left, bounds.top()),
            egui::vec2(width, bounds.height()),
        )
    } else {
        let height = bounds.width() / aspect;
        let top = bounds.center().y - height * 0.5;
        egui::Rect::from_min_size(
            egui::pos2(bounds.left(), top),
            egui::vec2(bounds.width(), height),
        )
    }
}

fn grid_reference_rect(rect: egui::Rect) -> egui::Rect {
    inset_rect(rect, GRID_REFERENCE_INSET_RATIO, GRID_REFERENCE_INSET_RATIO)
}

fn inset_rect(rect: egui::Rect, inset_x: f32, inset_y: f32) -> egui::Rect {
    egui::Rect::from_min_max(
        egui::pos2(
            rect.left() + rect.width() * inset_x,
            rect.top() + rect.height() * inset_y,
        ),
        egui::pos2(
            rect.right() - rect.width() * inset_x,
            rect.bottom() - rect.height() * inset_y,
        ),
    )
}

fn reference_aspect_ratio(state: &ManualCalibrationState) -> f32 {
    match state.kind {
        CodeKind::DataMatrix => {
            state.data_matrix_symbol.cols as f32 / state.data_matrix_symbol.rows.max(1) as f32
        }
        CodeKind::Qr | CodeKind::WxMiniprogram | CodeKind::Douyin | CodeKind::Unknown => 1.0,
    }
}

fn project_unit_point(h: &Matrix3<f64>, u: f32, v: f32) -> egui::Pos2 {
    let u = f64::from(u);
    let v = f64::from(v);
    let x = h[(0, 0)] * u + h[(0, 1)] * v + h[(0, 2)];
    let y = h[(1, 0)] * u + h[(1, 1)] * v + h[(1, 2)];
    let z = h[(2, 0)] * u + h[(2, 1)] * v + h[(2, 2)];
    if z.abs() <= f64::EPSILON {
        egui::Pos2::ZERO
    } else {
        egui::pos2((x / z) as f32, (y / z) as f32)
    }
}

fn default_corners(image_size: (u32, u32)) -> [egui::Pos2; 4] {
    let width = image_size.0.max(1) as f32;
    let height = image_size.1.max(1) as f32;
    let max_side = 0.94;
    let (display_w, display_h) = if width >= height {
        (max_side, max_side * height / width)
    } else {
        (max_side * width / height, max_side)
    };
    let left = (1.0 - display_w) * 0.5;
    let top = (1.0 - display_h) * 0.5;
    let right = left + display_w;
    let bottom = top + display_h;

    [
        egui::pos2(left, top),
        egui::pos2(right, top),
        egui::pos2(left, bottom),
        egui::pos2(right, bottom),
    ]
}

fn dy_locator_distance_norm() -> f32 {
    let margin = DY_MARGIN;
    ((0.5_f32 - margin).powi(2) * 2.0).sqrt()
}

fn qr_version_label(version: u8) -> String {
    let modules = qr_modules_for_version(version);
    format!("V{version} / {modules} x {modules}")
}

fn data_matrix_symbol_label(symbol: DataMatrixSymbol) -> String {
    let region_w = symbol.region_cols + 2;
    let region_h = symbol.region_rows + 2;
    let regions_x = symbol.cols / region_w.max(1);
    let regions_y = symbol.rows / region_h.max(1);
    if regions_x > 1 || regions_y > 1 {
        format!(
            "{} x {} / {} x {} regions",
            symbol.cols, symbol.rows, regions_x, regions_y
        )
    } else {
        format!("{} x {}", symbol.cols, symbol.rows)
    }
}

fn normalize_kind(kind: CodeKind) -> CodeKind {
    if kind.can_manual_calibrate() {
        kind
    } else {
        CodeKind::Douyin
    }
}
