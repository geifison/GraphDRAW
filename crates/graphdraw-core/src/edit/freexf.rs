//! Transformação livre (GD-FER-03): girar, refletir, escalar e inclinar a
//! seleção em torno de um ponto (o pivô, onde o arrasto começa), pela
//! posição do cursor.

use kurbo::{Affine, Point, Rect, Vec2};

/// Modo da transformação livre.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    /// Ângulo da reta pivô → cursor (Shift: passos de 15°).
    #[default]
    Rotate,
    /// Espelha na reta pivô → cursor (Shift: passos de 15°).
    Reflect,
    /// Escala pela distância ao pivô, tendo como referência o canto da
    /// seleção mais longe dele (Shift: x e y separados).
    Scale,
    /// Inclina na horizontal (arrasto mais lateral) ou na vertical, pela
    /// altura/largura da seleção.
    Skew,
}

fn snap_angle(a: f64, on: bool) -> f64 {
    if on {
        let step = 15f64.to_radians();
        (a / step).round() * step
    } else {
        a
    }
}

/// Matriz (página) da transformação livre: `pivot`, cursor `cur`, caixa da
/// seleção `bounds`; `shift` restringe. Arrasto curto demais (`min`, µm)
/// dá a identidade.
pub fn matrix(mode: Mode, pivot: Point, cur: Point, bounds: Rect, shift: bool, min: f64) -> Affine {
    let d = cur - pivot;
    if d.hypot() < min {
        return Affine::IDENTITY;
    }
    let about =
        |m: Affine| Affine::translate(pivot.to_vec2()) * m * Affine::translate(-pivot.to_vec2());
    match mode {
        Mode::Rotate => about(Affine::rotate(snap_angle(d.y.atan2(d.x), shift))),
        Mode::Reflect => {
            let a = snap_angle(d.y.atan2(d.x), shift);
            about(Affine::rotate(a) * Affine::scale_non_uniform(1.0, -1.0) * Affine::rotate(-a))
        }
        Mode::Scale => {
            let far = [
                Point::new(bounds.x0, bounds.y0),
                Point::new(bounds.x1, bounds.y0),
                Point::new(bounds.x0, bounds.y1),
                Point::new(bounds.x1, bounds.y1),
            ]
            .into_iter()
            .max_by(|a, b| (*a - pivot).hypot2().total_cmp(&(*b - pivot).hypot2()))
            .unwrap_or(cur);
            let r: Vec2 = far - pivot;
            if r.hypot2() <= 0.0 {
                return Affine::IDENTITY;
            }
            if shift {
                let sx = if r.x.abs() > min { d.x / r.x } else { 1.0 };
                let sy = if r.y.abs() > min { d.y / r.y } else { 1.0 };
                about(Affine::scale_non_uniform(sx, sy))
            } else {
                about(Affine::scale(d.dot(r) / r.hypot2()))
            }
        }
        Mode::Skew => {
            let (w, h) = (bounds.width().max(1.0), bounds.height().max(1.0));
            if d.x.abs() >= d.y.abs() {
                about(Affine::new([1.0, 0.0, d.x / h, 1.0, 0.0, 0.0]))
            } else {
                about(Affine::new([1.0, d.y / w, 0.0, 1.0, 0.0, 0.0]))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// GD-FER-03: o pivô fica parado; cada modo faz a sua transformação.
    #[test]
    fn gd_fer_03_free_transform_modes() {
        let b = Rect::new(0.0, 0.0, 100.0, 50.0);
        let pivot = Point::new(0.0, 0.0);
        // Girar 90°: cursor acima do pivô.
        let m = matrix(Mode::Rotate, pivot, Point::new(0.0, 30.0), b, false, 1.0);
        assert!((m * Point::new(10.0, 0.0) - Point::new(0.0, 10.0)).hypot() < 1e-9);
        assert!((m * pivot - pivot).hypot() < 1e-9);
        // Shift prende a 15°: 50° vira 45°.
        let a = 50f64.to_radians();
        let m = matrix(
            Mode::Rotate,
            pivot,
            Point::new(a.cos() * 30.0, a.sin() * 30.0),
            b,
            true,
            1.0,
        );
        let q = m * Point::new(10.0, 0.0);
        assert!((q.y.atan2(q.x).to_degrees() - 45.0).abs() < 1e-9);
        // Refletir na horizontal: y troca de sinal.
        let m = matrix(Mode::Reflect, pivot, Point::new(30.0, 0.0), b, false, 1.0);
        assert!((m * Point::new(10.0, 20.0) - Point::new(10.0, -20.0)).hypot() < 1e-9);
        // Escalar: cursor no meio do canto mais longe → metade.
        let m = matrix(Mode::Scale, pivot, Point::new(50.0, 25.0), b, false, 1.0);
        assert!((m * Point::new(100.0, 50.0) - Point::new(50.0, 25.0)).hypot() < 1e-9);
        // Shift: x e y separados.
        let m = matrix(Mode::Scale, pivot, Point::new(200.0, 25.0), b, true, 1.0);
        assert!((m * Point::new(100.0, 50.0) - Point::new(200.0, 25.0)).hypot() < 1e-9);
        // Inclinar: arrasto lateral desloca o topo.
        let m = matrix(Mode::Skew, pivot, Point::new(25.0, 5.0), b, false, 1.0);
        assert!((m * Point::new(0.0, 50.0) - Point::new(25.0, 50.0)).hypot() < 1e-9);
        // Arrasto curto: nada.
        assert_eq!(
            matrix(Mode::Rotate, pivot, Point::new(0.1, 0.0), b, false, 1.0),
            Affine::IDENTITY
        );
    }
}
