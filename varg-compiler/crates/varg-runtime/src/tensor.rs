// Wave 36: ndarray-backed Tensor builtins
// TensorHandle = Arc<ArrayD<f32>> — immutable, cheap to clone.
// All mutating ops return a new Arc rather than mutating in place.

use ndarray::{ArrayD, IxDyn, Axis, s};
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

pub fn __varg_tensor_from_list(data: &[f32], shape: &[i64]) -> TensorHandle {
    let s: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
    Arc::new(ArrayD::from_shape_vec(IxDyn(&s), data.to_vec())
        .expect("tensor_from_list: data length must match product of shape dims"))
}

// ── Shape ─────────────────────────────────────────────────────────────────────

pub fn __varg_tensor_shape(t: &TensorHandle) -> Vec<i64> {
    t.shape().iter().map(|&d| d as i64).collect()
}

pub fn __varg_tensor_reshape(t: &TensorHandle, shape: &[i64]) -> TensorHandle {
    let s: Vec<usize> = shape.iter().map(|&d| d as usize).collect();
    Arc::new(t.clone().into_shape_with_order(IxDyn(&s))
        .expect("tensor_reshape: new shape must have the same total element count"))
}

pub fn __varg_tensor_slice(t: &TensorHandle, dim: i64, start: i64, end: i64) -> TensorHandle {
    let ax = Axis(dim as usize);
    let sl = t.slice_axis(ax, ndarray::Slice::from((start as usize)..(end as usize)));
    Arc::new(sl.to_owned())
}

// ── Arithmetic ────────────────────────────────────────────────────────────────

pub fn __varg_tensor_add(a: &TensorHandle, b: &TensorHandle) -> TensorHandle {
    Arc::new(a.as_ref() + b.as_ref())
}

pub fn __varg_tensor_sub(a: &TensorHandle, b: &TensorHandle) -> TensorHandle {
    Arc::new(a.as_ref() - b.as_ref())
}

pub fn __varg_tensor_mul_scalar(t: &TensorHandle, s: f32) -> TensorHandle {
    Arc::new(t.mapv(|v| v * s))
}

pub fn __varg_tensor_matmul(a: &TensorHandle, b: &TensorHandle) -> TensorHandle {
    let a2 = a.view().into_dimensionality::<ndarray::Ix2>()
        .expect("tensor_matmul: both tensors must be rank-2 matrices");
    let b2 = b.view().into_dimensionality::<ndarray::Ix2>()
        .expect("tensor_matmul: both tensors must be rank-2 matrices");
    Arc::new(a2.dot(&b2).into_dyn())
}

pub fn __varg_tensor_dot(a: &TensorHandle, b: &TensorHandle) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

// ── Reductions ────────────────────────────────────────────────────────────────

pub fn __varg_tensor_sum(t: &TensorHandle) -> f32 {
    t.sum()
}

pub fn __varg_tensor_mean(t: &TensorHandle) -> f32 {
    if t.is_empty() { return 0.0; }
    t.sum() / t.len() as f32
}

pub fn __varg_tensor_max(t: &TensorHandle) -> f32 {
    t.iter().cloned().fold(f32::NEG_INFINITY, f32::max)
}

pub fn __varg_tensor_min(t: &TensorHandle) -> f32 {
    t.iter().cloned().fold(f32::INFINITY, f32::min)
}

// ── Conversion ────────────────────────────────────────────────────────────────

pub fn __varg_tensor_to_list(t: &TensorHandle) -> Vec<f32> {
    t.iter().cloned().collect()
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
        let t = __varg_tensor_from_list(&data, &[2, 3]);
        assert_eq!(__varg_tensor_to_list(&t), data);
    }

    #[test]
    fn test_tensor_reshape_preserves_elements() {
        let t = __varg_tensor_from_list(&[1.0, 2.0, 3.0, 4.0, 5.0, 6.0], &[6]);
        let r = __varg_tensor_reshape(&t, &[2, 3]);
        assert_eq!(__varg_tensor_shape(&r), vec![2, 3]);
        assert_eq!(__varg_tensor_to_list(&r).len(), 6);
    }

    #[test]
    fn test_tensor_matmul_identity() {
        let eye = __varg_tensor_eye(2);
        let m = __varg_tensor_from_list(&[1.0, 2.0, 3.0, 4.0], &[2, 2]);
        let result = __varg_tensor_matmul(&eye, &m);
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
        let c = __varg_tensor_add(&a, &b);
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
        let t = __varg_tensor_from_list(&[1.0,2.0,3.0,4.0,5.0,6.0], &[3, 2]);
        let sl = __varg_tensor_slice(&t, 0, 1, 3);
        assert_eq!(__varg_tensor_shape(&sl), vec![2, 2]);
    }

    #[test]
    fn test_tensor_max_min() {
        let t = __varg_tensor_from_list(&[3.0, 1.0, 4.0, 1.0, 5.0], &[5]);
        assert_eq!(__varg_tensor_max(&t), 5.0);
        assert_eq!(__varg_tensor_min(&t), 1.0);
    }

    #[test]
    fn test_tensor_dot() {
        let a = __varg_tensor_from_list(&[1.0, 2.0, 3.0], &[3]);
        let b = __varg_tensor_from_list(&[4.0, 5.0, 6.0], &[3]);
        assert!((__varg_tensor_dot(&a, &b) - 32.0).abs() < 1e-6);
    }
}
