use crate::app::QRacerApp;
use crate::code_kind::CodeKind;
use crate::pipeline::perspective::homography_from_4pts;
use eframe::egui;
use nalgebra::Matrix3;

const CANVAS_PADDING_RATIO: f32 = 0.08;
const HANDLE_RADIUS: f32 = 6.0;
const HANDLE_HIT_RADIUS: f32 = 16.0;
const ROTATE_HANDLE_RADIUS: f32 = 7.0;
const ROTATE_HANDLE_HIT_RADIUS: f32 = 18.0;
const ROTATE_HANDLE_OFFSET: f32 = 22.0;
const ROTATION_STEP_DEGREES: f32 = 1.0;
const IMAGE_MESH_STEPS: usize = 18;
const UNDO_LIMIT: usize = 80;
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

#[derive(Debug)]
pub struct ManualCalibrationState {
    pub open: bool,
    pub kind: CodeKind,
    corners: [egui::Pos2; 4],
    drag: Option<DragTarget>,
    last_pointer: Option<egui::Pos2>,
    drag_undo_pushed: bool,
    scroll_undo_pushed: bool,
    image_size: Option<(u32, u32)>,
    undo_stack: Vec<[egui::Pos2; 4]>,
}

#[derive(Debug, Clone, Copy)]
enum DragTarget {
    Corner(usize),
    Move,
    Rotate,
}

impl ManualCalibrationState {
    pub fn new() -> Self {
        Self {
            open: false,
            kind: CodeKind::Douyin,
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

    pub fn close_for_new_image(&mut self) {
        self.open = false;
        self.drag = None;
        self.last_pointer = None;
        self.drag_undo_pushed = false;
        self.scroll_undo_pushed = false;
        self.image_size = None;
        self.undo_stack.clear();
    }

    pub fn output_corners(&self, target_size: u32) -> [(f64, f64); 4] {
        let max = target_size.saturating_sub(1) as f64;
        self.corners
            .map(|corner| (f64::from(corner.x) * max, f64::from(corner.y) * max))
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

    if let Some(kind) = app
        .current_forced_or_detected_kind()
        .filter(|kind| kind.can_process())
    {
        app.manual_calibration.set_kind(kind);
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
    let dy_has_border = app
        .last_dy_grid
        .as_ref()
        .is_some_and(|grid| grid.has_border);

    egui::Window::new("手动校准")
        .open(&mut open)
        .resizable(true)
        .default_size(egui::vec2(860.0, 820.0))
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
            ui.horizontal_wrapped(|ui| {
                ui.label("操作：拖动图片可整体移动；拖动四角黄色点可透视变形；拖动画面上方圆点可旋转；滚轮可缩放；Ctrl+Z 可撤销上一步。");
            });
            ui.separator();

            calibration_canvas(ui, &texture, &mut app.manual_calibration, dy_has_border);
        });

    if close_requested {
        open = false;
    }
    app.manual_calibration.open = open;
    if let Some(kind) = selected_kind {
        app.set_code_kind_override(Some(kind));
        app.manual_calibration.set_kind(kind);
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
    dy_has_border: bool,
) {
    let available = ui.available_size();
    let side = available.x.min(available.y.max(420.0)).max(360.0);
    let (rect, response) =
        ui.allocate_exact_size(egui::vec2(side, side), egui::Sense::click_and_drag());
    let reference_rect = rect.shrink(side * CANVAS_PADDING_RATIO);
    let painter = ui.painter_at(rect);

    painter.rect_filled(rect, 6.0, egui::Color32::from_rgb(32, 34, 36));
    painter.rect_filled(reference_rect, 0.0, egui::Color32::from_rgb(248, 248, 246));

    draw_transformed_image(&painter, texture, state, reference_rect);
    draw_reference(&painter, reference_rect, state.kind, dy_has_border);
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

fn draw_reference(painter: &egui::Painter, rect: egui::Rect, kind: CodeKind, dy_has_border: bool) {
    painter.rect_stroke(
        rect,
        0.0,
        egui::Stroke::new(1.5, egui::Color32::from_rgb(40, 170, 210)),
        egui::StrokeKind::Inside,
    );

    match kind {
        CodeKind::WxMiniprogram => draw_wx_reference(painter, rect),
        CodeKind::Douyin => draw_dy_reference(painter, rect, dy_has_border),
        CodeKind::Unknown | CodeKind::Qr | CodeKind::DataMatrix => {}
    }
}

fn draw_wx_reference(painter: &egui::Painter, rect: egui::Rect) {
    let margin = 0.23;
    let far = 1.0 - margin;
    let leg = far - margin;
    let center = egui::pos2(0.5, 0.5);
    let locator_radius = leg * 0.0786;
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
            let factor = (1.0_f32 + scroll * 0.001).clamp(0.88, 1.14);
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
            match target {
                DragTarget::Corner(index) => state.corners[index] += delta,
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
    point_in_quad(pointer, points).then_some(DragTarget::Move)
}

fn scale_about(state: &mut ManualCalibrationState, pivot: egui::Pos2, factor: f32) {
    for corner in &mut state.corners {
        let offset = *corner - pivot;
        *corner = pivot + offset * factor;
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

fn normalize_kind(kind: CodeKind) -> CodeKind {
    if kind.can_manual_calibrate() {
        kind
    } else {
        CodeKind::Douyin
    }
}
