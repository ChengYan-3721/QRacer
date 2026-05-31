#[allow(dead_code)]
pub fn polar_sector_path(
    cx: f64,
    cy: f64,
    r_inner: f64,
    r_outer: f64,
    theta_start: f64,
    theta_end: f64,
) -> String {
    let outer_start = polar_point(cx, cy, r_outer, theta_start);
    let outer_end = polar_point(cx, cy, r_outer, theta_end);
    let inner_end = polar_point(cx, cy, r_inner, theta_end);
    let inner_start = polar_point(cx, cy, r_inner, theta_start);
    let large_arc = i32::from((theta_end - theta_start).abs() > std::f64::consts::PI);

    if r_inner <= 0.0 {
        return format!(
            "M {cx:.3} {cy:.3} L {:.3} {:.3} A {r_outer:.3} {r_outer:.3} 0 {large_arc} 1 {:.3} {:.3} Z",
            outer_start.0, outer_start.1, outer_end.0, outer_end.1
        );
    }

    format!(
        "M {:.3} {:.3} A {r_outer:.3} {r_outer:.3} 0 {large_arc} 1 {:.3} {:.3} L {:.3} {:.3} A {r_inner:.3} {r_inner:.3} 0 {large_arc} 0 {:.3} {:.3} Z",
        outer_start.0,
        outer_start.1,
        outer_end.0,
        outer_end.1,
        inner_end.0,
        inner_end.1,
        inner_start.0,
        inner_start.1
    )
}

#[allow(dead_code)]
fn polar_point(cx: f64, cy: f64, radius: f64, theta: f64) -> (f64, f64) {
    (cx + radius * theta.cos(), cy + radius * theta.sin())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sector_path_contains_arc_commands() {
        let path = polar_sector_path(10.0, 10.0, 2.0, 4.0, 0.0, std::f64::consts::FRAC_PI_2);

        assert!(path.starts_with("M "));
        assert!(path.contains(" A "));
        assert!(path.ends_with(" Z"));
    }
}
