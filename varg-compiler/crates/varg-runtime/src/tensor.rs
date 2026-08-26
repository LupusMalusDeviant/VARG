// Wave 36: ndarray-backed Tensor builtins
// TensorHandle = Arc<ArrayD<f32>> — immutable, cheap to clone.
// All mutating ops return a new Arc rather than mutating in place.

use ndarray::{ArrayD, IxDyn, Axis};
use std::sync::Arc;

pub type TensorHandle = Arc<ArrayD<f32>>;

// ── Construction ──────────────────────────────────────────────────────────────

pub fn __varg_tensor_zeros(shape: &[i64]) -> TensorHandle {
    let s: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
    Arc::new(ArrayD::zeros(IxDyn(&s)))
}

pub fn __varg_tensor_ones(shape: &[i64]) -> TensorHandle {
    let s: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
    Arc::new(ArrayD::ones(IxDyn(&s)))
}

pub fn __varg_tensor_eye(n: i64) -> TensorHandle {
    let n = n as usize;
    let mut a = ArrayD::zeros(IxDyn(&[n, n]));
    for i in 0..n {
        a[[i, i]] = 1.0;
    }
    Arc::new(a)
}

/// Build a tensor from a flat list.
///
/// Takes anything convertible to `Vec<f32>` rather than a hard `&[f32]`: Varg float literals
/// compile to `f64`, so the documented `tensor_from_list([1.0, 2.0], [2])` could not be called
/// from Varg at all. This is the same fix the vector store already carries — hence the shared
/// `ToF32Vec` trait rather than a second conversion.
pub fn __varg_tensor_from_list<D: crate::vector::ToF32Vec + ?Sized>(
    data: &D,
    shape: &[i64],
) -> Result<TensorHandle, String> {
    let s: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
    let values = data.to_f32_vec();
    let wanted: usize = s.iter().product();
    if values.len() != wanted {
        return Err(format!(
            "tensor_from_list: shape {:?} needs {} values, got {}",
            shape,
            wanted,
            values.len()
        ));
    }
    ArrayD::from_shape_vec(IxDyn(&s), values)
        .map(Arc::new)
        .map_err(|e| format!("tensor_from_list: {}", e))
}

// ── Shape ─────────────────────────────────────────────────────────────────────

pub fn __varg_tensor_shape(t: &TensorHandle) -> Vec<i64> {
    t.shape().iter().map(|&d| d as i64).collect()
}

pub fn __varg_tensor_reshape(t: &TensorHandle, shape: &[i64]) -> Result<TensorHandle, String> {
    let s: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
    let wanted: usize = s.iter().product();
    if t.len() != wanted {
        return Err(format!(
            "tensor_reshape: shape {:?} holds {} values, this tensor has {}",
            shape,
            wanted,
            t.len()
        ));
    }
    // `into_shape_with_order` consumes an owned array; clone the inner ArrayD out of the Arc
    // (not the Arc itself — that can't be moved out of).
    (**t).clone()
        .into_shape_with_order(IxDyn(&s))
        .map(Arc::new)
        .map_err(|e| format!("tensor_reshape: {}", e))
}

pub fn __varg_tensor_slice(
    t: &TensorHandle,
    dim: i64,
    start: i64,
    end: i64,
) -> Result<TensorHandle, String> {
    // ndarray panics on a bad axis or range, from inside its own slicing code, so the message
    // named neither the tensor nor the numbers the caller passed.
    let shape = t.shape();
    if dim < 0 || dim as usize >= shape.len() {
        return Err(format!(
            "tensor_slice: axis {} does not exist on a rank-{} tensor",
            dim,
            shape.len()
        ));
    }
    let len = shape[dim as usize] as i64;
    if start < 0 || end < start || end > len {
        return Err(format!(
            "tensor_slice: range {}..{} does not fit axis {}, which has {} elements",
            start, end, dim, len
        ));
    }
    let ax = Axis(dim as usize);
    let sl = t.slice_axis(ax, ndarray::Slice::from((start as usize)..(end as usize)));
    Ok(Arc::new(sl.to_owned()))
}

/// Do two tensors have the same shape? Element-wise arithmetic needs that, and ndarray's `+`
/// panics when they do not — from inside its own operator, with "IncompatibleShape" and nothing
/// about which two tensors were involved.
fn same_shape(a: &TensorHandle, b: &TensorHandle, op: &str) -> Result<(), String> {
    if a.shape() == b.shape() {
        return Ok(());
    }
    Err(format!(
        "tensor_{}: shapes {:?} and {:?} do not match",
        op,
        a.shape(),
        b.shape()
    ))
}

// ── Arithmetic ────────────────────────────────────────────────────────────────

pub fn __varg_tensor_add(a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle, String> {
    same_shape(a, b, "add")?;
    Ok(Arc::new(a.as_ref() + b.as_ref()))
}

pub fn __varg_tensor_sub(a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle, String> {
    same_shape(a, b, "sub")?;
    Ok(Arc::new(a.as_ref() - b.as_ref()))
}

// The tensor API speaks f64 at the Varg boundary even though ndarray stores f32 internally.
// Varg's `float` is f64, so f32 in these signatures meant `tensor_mul_scalar(t, 2.0)` did not
// compile and the reductions' results could not be combined with any other Varg float — the
// builtin signature table has always claimed Float (f64) for them.
pub fn __varg_tensor_mul_scalar(t: &TensorHandle, s: f64) -> TensorHandle {
    let s32 = s as f32;
    Arc::new(t.mapv(|v| v * s32))
}

pub fn __varg_tensor_matmul(a: &TensorHandle, b: &TensorHandle) -> Result<TensorHandle, String> {
    let a2 = a.view().into_dimensionality::<ndarray::Ix2>()
        .map_err(|_| format!("tensor_matmul: left tensor is rank {}, needs rank 2", a.ndim()))?;
    let b2 = b.view().into_dimensionality::<ndarray::Ix2>()
        .map_err(|_| format!("tensor_matmul: right tensor is rank {}, needs rank 2", b.ndim()))?;
    if a2.ncols() != b2.nrows() {
        return Err(format!(
            "tensor_matmul: {}x{} cannot multiply {}x{} — the inner dimensions must match",
            a2.nrows(), a2.ncols(), b2.nrows(), b2.ncols()
        ));
    }
    Ok(Arc::new(a2.dot(&b2).into_dyn()))
}

pub fn __varg_tensor_dot(a: &TensorHandle, b: &TensorHandle) -> f64 {
    let acc: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    acc as f64
}

// ── Reductions ────────────────────────────────────────────────────────────────

pub fn __varg_tensor_sum(t: &TensorHandle) -> f64 {
    t.sum() as f64
}

pub fn __varg_tensor_mean(t: &TensorHandle) -> f64 {
    if t.is_empty() { return 0.0; }
    (t.sum() / t.len() as f32) as f64
}

pub fn __varg_tensor_max(t: &TensorHandle) -> f64 {
    t.iter().cloned().fold(f32::NEG_INFINITY, f32::max) as f64
}

pub fn __varg_tensor_min(t: &TensorHandle) -> f64 {
    t.iter().cloned().fold(f32::INFINITY, f32::min) as f64
}

// ── Conversion ────────────────────────────────────────────────────────────────

pub fn __varg_tensor_to_list(t: &TensorHandle) -> Vec<f64> {
    t.iter().map(|v| *v as f64).collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tensor_zeros_shape() {
        let t = __varg_tensor_zeros(&[2, 3]);
        assert_eq!(__varg_tensor_shape(&t), vec![2, 3]);
        assert_eq!(__varg_tensor_sum(&t), 0.0);
    }

    #[test]
    fn test_tensor_ones() {
        let t = __varg_tensor_ones(&[3]);
        assert_eq!(__varg_tensor_sum(&t), 3.0);
    }

    #[test]
    fn test_tensor_eye_identity() {
        let eye = __varg_tensor_eye(3);
        assert_eq!(__varg_tensor_shape(&eye), vec![3, 3]);
        assert_eq!(__varg_tensor_sum(&eye), 3.0);
    }

    #[test]
    fn test_tensor_from_list_roundtrip() {
        let data = vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0];
        let t = __varg_tensor_from_list(&data, &[2, 3]).unwrap();
        assert_eq!(__varg_tensor_to_list(&t), data);
    }

    #[test]
    fn test_tensor_reshape_preserves_elements() {
        let t = __varg_tensor_from_list(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[6]).unwrap();
        let r = __varg_tensor_reshape(&t, &[2, 3]).unwrap();
        assert_eq!(__varg_tensor_shape(&r), vec![2, 3]);
        assert_eq!(__varg_tensor_to_list(&r).len(), 6);
    }

    #[test]
    fn test_tensor_matmul_identity() {
        let eye = __varg_tensor_eye(2);
        let m = __varg_tensor_from_list(&[1.0, 2.0, 3.0, 4.0], &[2, 2]).unwrap();
        let result = __varg_tensor_matmul(&eye, &m).unwrap();
        let expected = __varg_tensor_to_list(&m);
        let got = __varg_tensor_to_list(&result);
        for (a, b) in expected.iter().zip(got.iter()) {
            assert!((a - b).abs() < 1e-6, "matmul identity failed: {} vs {}", a, b);
        }
    }

    #[test]
    fn test_tensor_add_elementwise() {
        let a = __varg_tensor_ones(&[3]);
        let b = __varg_tensor_ones(&[3]);
        let c = __varg_tensor_add(&a, &b).unwrap();
        assert_eq!(__varg_tensor_sum(&c), 6.0);
    }

    #[test]
    fn test_tensor_mul_scalar() {
        let t = __varg_tensor_ones(&[4]);
        let r = __varg_tensor_mul_scalar(&t, 3.0);
        assert_eq!(__varg_tensor_sum(&r), 12.0);
    }

    #[test]
    fn test_tensor_mean_empty() {
        let t = __varg_tensor_zeros(&[0]);
        assert_eq!(__varg_tensor_mean(&t), 0.0);
    }

    #[test]
    fn test_tensor_slice_axis0() {
        let t = __varg_tensor_from_list(&[1.0,2.0,3.0,4.0,5.0,6.0], &[3, 2]).unwrap();
        let sl = __varg_tensor_slice(&t, 0, 1, 3).unwrap();
        assert_eq!(__varg_tensor_shape(&sl), vec![2, 2]);
    }

    #[test]
    fn test_tensor_max_min() {
        let t = __varg_tensor_from_list(&[3.0, 1.0, 4.0, 1.0, 5.0], &[5]).unwrap();
        assert_eq!(__varg_tensor_max(&t), 5.0);
        assert_eq!(__varg_tensor_min(&t), 1.0);
    }

    #[test]
    fn test_tensor_dot() {
        let a = __varg_tensor_from_list(&[1.0, 2.0, 3.0], &[3]).unwrap();
        let b = __varg_tensor_from_list(&[4.0, 5.0, 6.0], &[3]).unwrap();
        assert!((__varg_tensor_dot(&a, &b) - 32.0).abs() < 1e-6);
    }
}
