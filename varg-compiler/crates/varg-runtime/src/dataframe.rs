// Wave 38: Polars DataFrame builtins
// DataFrameHandle = Arc<Mutex<DataFrame>>
// Uses Polars lazy API internally for filter/groupby/agg/sort (query optimisation).

use polars::prelude::*;
use std::sync::{Arc, Mutex};

pub type DataFrameHandle = Arc<Mutex<DataFrame>>;

// ── I/O ───────────────────────────────────────────────────────────────────────

pub fn __varg_df_read_csv(path: &str) -> DataFrameHandle {
    let df = CsvReadOptions::default()
        .with_infer_schema_length(Some(100))
        .try_into_reader_with_file_path(Some(path.into()))
        .expect("df_read_csv: could not open file")
        .finish()
        .expect("df_read_csv: could not parse CSV");
    Arc::new(Mutex::new(df))
}

pub fn __varg_df_read_parquet(path: &str) -> DataFrameHandle {
    let file = std::fs::File::open(path)
        .expect("df_read_parquet: could not open file");
    let df = ParquetReader::new(file)
        .finish()
        .expect("df_read_parquet: could not parse Parquet");
    Arc::new(Mutex::new(df))
}

pub fn __varg_df_write_csv(df: &DataFrameHandle, path: &str) {
    let mut file = std::fs::File::create(path)
        .expect("df_write_csv: could not create file");
    CsvWriter::new(&mut file)
        .finish(&mut df.lock().unwrap().clone())
        .expect("df_write_csv: write failed");
}

pub fn __varg_df_write_parquet(df: &DataFrameHandle, path: &str) {
    let file = std::fs::File::create(path)
        .expect("df_write_parquet: could not create file");
    ParquetWriter::new(file)
        .finish(&mut df.lock().unwrap().clone())
        .expect("df_write_parquet: write failed");
}

// ── Transformation ────────────────────────────────────────────────────────────

pub fn __varg_df_select(df: &DataFrameHandle, cols: &[String]) -> DataFrameHandle {
    let col_exprs: Vec<Expr> = cols.iter().map(|c| col(c)).collect();
    let result = df.lock().unwrap()
        .clone()
        .lazy()
        .select(col_exprs)
        .collect()
        .expect("df_select: failed");
    Arc::new(Mutex::new(result))
}

pub fn __varg_df_filter(df: &DataFrameHandle, expr_str: &str) -> DataFrameHandle {
    // Supports simple "col_name op value" strings, e.g. "age > 30", "name == Alice"
    let filter_expr = parse_simple_filter(expr_str);
    let result = df.lock().unwrap()
        .clone()
        .lazy()
        .filter(filter_expr)
        .collect()
        .expect("df_filter: failed");
    Arc::new(Mutex::new(result))
}

fn parse_simple_filter(expr_str: &str) -> Expr {
    let parts: Vec<&str> = expr_str.splitn(3, ' ').collect();
    if parts.len() != 3 {
        panic!("df_filter: expected \"col op value\", got {:?}", expr_str);
    }
    let (col_name, op, val_str) = (parts[0], parts[1], parts[2]);
    let val_str = val_str.trim_matches('"');
    // Try numeric first, fall back to string
    let value_expr: Expr = if let Ok(v) = val_str.parse::<f64>() {
        lit(v)
    } else if let Ok(v) = val_str.parse::<i64>() {
        lit(v)
    } else {
        lit(val_str.to_string())
    };
    let c = col(col_name);
    match op {
        "==" => c.eq(value_expr),
        "!=" => c.neq(value_expr),
        ">"  => c.gt(value_expr),
        ">=" => c.gt_eq(value_expr),
        "<"  => c.lt(value_expr),
        "<=" => c.lt_eq(value_expr),
        _    => panic!("df_filter: unsupported operator {:?}", op),
    }
}

pub fn __varg_df_sort(df: &DataFrameHandle, col_name: &str, ascending: bool) -> DataFrameHandle {
    let result = df.lock().unwrap()
        .clone()
        .lazy()
        .sort([col_name], SortMultipleOptions::default().with_order_descending(!ascending))
        .collect()
        .expect("df_sort: failed");
    Arc::new(Mutex::new(result))
}

pub fn __varg_df_groupby(df: &DataFrameHandle, by_cols: &[String]) -> DataFrameHandle {
    // groupby without agg returns the dataframe sorted by group columns
    let exprs: Vec<Expr> = by_cols.iter().map(|c| col(c)).collect();
    let result = df.lock().unwrap()
        .clone()
        .lazy()
        .sort(by_cols.iter().map(|s| s.as_str()).collect::<Vec<_>>(), SortMultipleOptions::default())
        .collect()
        .expect("df_groupby: failed");
    let _ = exprs; // consumed via lazy above
    Arc::new(Mutex::new(result))
}

pub fn __varg_df_agg(df: &DataFrameHandle, by_cols: &[String], agg_fn: &str) -> DataFrameHandle {
    let group_exprs: Vec<Expr> = by_cols.iter().map(|c| col(c)).collect();
    // Apply agg_fn to all non-group columns
    let all_cols = col("*");
    let agg_expr: Expr = match agg_fn {
        "sum"   => all_cols.sum(),
        "mean"  => all_cols.mean(),
        "count" => all_cols.count(),
        "min"   => all_cols.min(),
        "max"   => all_cols.max(),
        other   => panic!("df_agg: unsupported aggregation {:?}. Use: sum, mean, count, min, max", other),
    };
    let result = df.lock().unwrap()
        .clone()
        .lazy()
        .group_by(group_exprs)
        .agg([agg_expr])
        .collect()
        .expect("df_agg: failed");
    Arc::new(Mutex::new(result))
}

pub fn __varg_df_head(df: &DataFrameHandle, n: i64) -> DataFrameHandle {
    let result = df.lock().unwrap().head(Some(n as usize));
    Arc::new(Mutex::new(result))
}

pub fn __varg_df_with_column(df: &DataFrameHandle, name: &str, data: &[f32]) -> DataFrameHandle {
    let series = Series::new(name.into(), data);
    let mut inner = df.lock().unwrap().clone();
    inner.with_column(series).expect("df_with_column: failed");
    Arc::new(Mutex::new(inner))
}

// ── Introspection ─────────────────────────────────────────────────────────────

pub fn __varg_df_shape(df: &DataFrameHandle) -> (i64, i64) {
    let inner = df.lock().unwrap();
    (inner.height() as i64, inner.width() as i64)
}

pub fn __varg_df_columns(df: &DataFrameHandle) -> Vec<String> {
    df.lock().unwrap()
        .get_column_names()
        .iter()
        .map(|s| s.to_string())
        .collect()
}

// ── Tests ─────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn sample_df() -> DataFrameHandle {
        let df = df!(
            "name"    => &["Alice", "Bob", "Carol", "Dave"],
            "age"     => &[30_i32, 25, 30, 40],
            "score"   => &[90.0_f64, 85.0, 92.0, 78.0]
        ).unwrap();
        Arc::new(Mutex::new(df))
    }

    #[test]
    fn test_df_shape() {
        let df = sample_df();
        assert_eq!(__varg_df_shape(&df), (4, 3));
    }

    #[test]
    fn test_df_columns() {
        let df = sample_df();
        let cols = __varg_df_columns(&df);
        assert!(cols.contains(&"name".to_string()));
        assert!(cols.contains(&"age".to_string()));
        assert!(cols.contains(&"score".to_string()));
    }

    #[test]
    fn test_df_select_reduces_columns() {
        let df = sample_df();
        let sel = __varg_df_select(&df, &["name".to_string(), "age".to_string()]);
        assert_eq!(__varg_df_shape(&sel).1, 2);
    }

    #[test]
    fn test_df_filter_equality() {
        let df = sample_df();
        let filtered = __varg_df_filter(&df, "age == 30");
        assert_eq!(__varg_df_shape(&filtered).0, 2);
    }

    #[test]
    fn test_df_filter_gt() {
        let df = sample_df();
        let filtered = __varg_df_filter(&df, "score > 88");
        assert_eq!(__varg_df_shape(&filtered).0, 2);
    }

    #[test]
    fn test_df_head_limits_rows() {
        let df = sample_df();
        let h = __varg_df_head(&df, 2);
        assert_eq!(__varg_df_shape(&h).0, 2);
    }

    #[test]
    fn test_df_sort_ascending() {
        let df = sample_df();
        let sorted = __varg_df_sort(&df, "age", true);
        let inner = sorted.lock().unwrap();
        let ages: Vec<i32> = inner.column("age").unwrap()
            .i32().unwrap().into_no_null_iter().collect();
        assert_eq!(ages[0], 25);
    }

    #[test]
    fn test_df_with_column_appends() {
        let df = sample_df();
        let data = vec![1.0_f32, 2.0, 3.0, 4.0];
        let extended = __varg_df_with_column(&df, "rank", &data);
        assert_eq!(__varg_df_shape(&extended).1, 4);
        assert!(__varg_df_columns(&extended).contains(&"rank".to_string()));
    }
}
