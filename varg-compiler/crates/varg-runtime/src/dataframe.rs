// Wave 38: Polars DataFrame builtins
// DataFrameHandle = Arc<Mutex<DataFrame>>
// Uses Polars lazy API internally for filter/groupby/agg/sort (query optimisation).

use polars::prelude::*;
use std::sync::{Arc, Mutex};

pub type DataFrameHandle = Arc<Mutex<DataFrame>>;

// ── I/O ───────────────────────────────────────────────────────────────────────

pub fn __varg_df_read_csv(path: &str) -> Result<DataFrameHandle, String> {
    let df = CsvReadOptions::default()
        .with_infer_schema_length(Some(100))
        .try_into_reader_with_file_path(Some(path.into()))
        .map_err(|e| format!("df_read_csv: could not open `{}`: {}", path, e))?
        .finish()
        .map_err(|e| format!("df_read_csv: `{}` is not readable as CSV: {}", path, e))?;
    Ok(Arc::new(Mutex::new(df)))
}

pub fn __varg_df_read_parquet(path: &str) -> Result<DataFrameHandle, String> {
    let file = std::fs::File::open(path)
        .map_err(|e| format!("df_read_parquet: could not open `{}`: {}", path, e))?;
    let df = ParquetReader::new(file)
        .finish()
        .map_err(|e| format!("df_read_parquet: `{}` is not readable as Parquet: {}", path, e))?;
    Ok(Arc::new(Mutex::new(df)))
}

pub fn __varg_df_write_csv(df: &DataFrameHandle, path: &str) -> Result<(), String> {
    let mut file = std::fs::File::create(path)
        .map_err(|e| format!("df_write_csv: could not create `{}`: {}", path, e))?;
    CsvWriter::new(&mut file)
        .finish(&mut df.lock().unwrap_or_else(|e| e.into_inner()).clone())
        .map_err(|e| format!("df_write_csv: writing `{}` failed: {}", path, e))
}

pub fn __varg_df_write_parquet(df: &DataFrameHandle, path: &str) -> Result<(), String> {
    let file = std::fs::File::create(path)
        .map_err(|e| format!("df_write_parquet: could not create `{}`: {}", path, e))?;
    ParquetWriter::new(file)
        .finish(&mut df.lock().unwrap_or_else(|e| e.into_inner()).clone())
        .map(|_| ())
        .map_err(|e| format!("df_write_parquet: writing `{}` failed: {}", path, e))
}

// ── Transformation ────────────────────────────────────────────────────────────

pub fn __varg_df_select(df: &DataFrameHandle, cols: &[String]) -> Result<DataFrameHandle, String> {
    let col_exprs: Vec<Expr> = cols.iter().map(|c| col(c)).collect();
    let result = df.lock().unwrap_or_else(|e| e.into_inner())
        .clone()
        .lazy()
        .select(col_exprs)
        .collect()
        .map_err(|e| format!("df_select: {}", e))?;
    Ok(Arc::new(Mutex::new(result)))
}

pub fn __varg_df_filter(df: &DataFrameHandle, expr_str: &str) -> Result<DataFrameHandle, String> {
    // Supports simple "col_name op value" strings, e.g. "age > 30", "name == Alice"
    let filter_expr = parse_simple_filter(expr_str)?;
    let result = df.lock().unwrap_or_else(|e| e.into_inner())
        .clone()
        .lazy()
        .filter(filter_expr)
        .collect()
        .map_err(|e| format!("df_filter: {}", e))?;
    Ok(Arc::new(Mutex::new(result)))
}

fn parse_simple_filter(expr_str: &str) -> Result<Expr, String> {
    // The expression and the operator come straight from the caller, so a typo in either used to
    // take the program down — the most ordinary mistake anyone makes writing a filter.
    let parts: Vec<&str> = expr_str.splitn(3, ' ').collect();
    if parts.len() != 3 {
        return Err(format!(
            "df_filter: expected \"column operator value\", got {:?}",
            expr_str
        ));
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
    Ok(match op {
        "==" => c.eq(value_expr),
        "!=" => c.neq(value_expr),
        ">"  => c.gt(value_expr),
        ">=" => c.gt_eq(value_expr),
        "<"  => c.lt(value_expr),
        "<=" => c.lt_eq(value_expr),
        _ => {
            return Err(format!(
                "df_filter: unknown operator {:?} — use ==, !=, >, >=, < or <=",
                op
            ))
        }
    })
}

pub fn __varg_df_sort(df: &DataFrameHandle, col_name: &str, ascending: bool) -> Result<DataFrameHandle, String> {
    let result = df.lock().unwrap_or_else(|e| e.into_inner())
        .clone()
        .lazy()
        .sort([col_name], SortMultipleOptions::default().with_order_descending(!ascending))
        .collect()
        .map_err(|e| format!("df_sort: {}", e))?;
    Ok(Arc::new(Mutex::new(result)))
}

pub fn __varg_df_groupby(df: &DataFrameHandle, by_cols: &[String]) -> Result<DataFrameHandle, String> {
    // groupby without agg returns the dataframe sorted by group columns
    let exprs: Vec<Expr> = by_cols.iter().map(|c| col(c)).collect();
    let result = df.lock().unwrap_or_else(|e| e.into_inner())
        .clone()
        .lazy()
        .sort(by_cols.iter().map(|s| s.as_str()).collect::<Vec<_>>(), SortMultipleOptions::default())
        .collect()
        .map_err(|e| format!("df_groupby: {}", e))?;
    let _ = exprs; // consumed via lazy above
    Ok(Arc::new(Mutex::new(result)))
}

pub fn __varg_df_agg(df: &DataFrameHandle, by_cols: &[String], agg_fn: &str) -> Result<DataFrameHandle, String> {
    let group_exprs: Vec<Expr> = by_cols.iter().map(|c| col(c)).collect();
    // Apply agg_fn to all non-group columns
    let all_cols = col("*");
    let agg_expr: Expr = match agg_fn {
        "sum"   => all_cols.sum(),
        "mean"  => all_cols.mean(),
        "count" => all_cols.count(),
        "min"   => all_cols.min(),
        "max"   => all_cols.max(),
        other => {
            return Err(format!(
                "df_agg: unknown aggregation {:?} — use sum, mean, count, min or max",
                other
            ))
        }
    };
    let result = df.lock().unwrap_or_else(|e| e.into_inner())
        .clone()
        .lazy()
        .group_by(group_exprs)
        .agg([agg_expr])
        .collect()
        .map_err(|e| format!("df_agg: {}", e))?;
    Ok(Arc::new(Mutex::new(result)))
}

pub fn __varg_df_head(df: &DataFrameHandle, n: i64) -> DataFrameHandle {
    let result = df.lock().unwrap_or_else(|e| e.into_inner()).head(Some(n as usize));
    Arc::new(Mutex::new(result))
}

pub fn __varg_df_with_column(
    df: &DataFrameHandle,
    name: &str,
    data: &[f32],
) -> Result<DataFrameHandle, String> {
    let series = Series::new(name.into(), data);
    let mut inner = df.lock().unwrap_or_else(|e| e.into_inner()).clone();
    inner
        .with_column(series)
        .map_err(|e| format!("df_with_column: {}", e))?;
    Ok(Arc::new(Mutex::new(inner)))
}

// ── Introspection ─────────────────────────────────────────────────────────────

pub fn __varg_df_shape(df: &DataFrameHandle) -> (i64, i64) {
    let inner = df.lock().unwrap_or_else(|e| e.into_inner());
    (inner.height() as i64, inner.width() as i64)
}

pub fn __varg_df_columns(df: &DataFrameHandle) -> Vec<String> {
    df.lock().unwrap_or_else(|e| e.into_inner())
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
        let sel = __varg_df_select(&df, &["name".to_string(), "age".to_string()]).unwrap();
        assert_eq!(__varg_df_shape(&sel).1, 2);
    }

    #[test]
    fn test_df_filter_equality() {
        let df = sample_df();
        let filtered = __varg_df_filter(&df, "age == 30").unwrap();
        assert_eq!(__varg_df_shape(&filtered).0, 2);
    }

    #[test]
    fn test_df_filter_gt() {
        let df = sample_df();
        let filtered = __varg_df_filter(&df, "score > 88").unwrap();
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
        let sorted = __varg_df_sort(&df, "age", true).unwrap();
        let inner = sorted.lock().unwrap_or_else(|e| e.into_inner());
        let ages: Vec<i32> = inner.column("age").unwrap()
            .i32().unwrap().into_no_null_iter().collect();
        assert_eq!(ages[0], 25);
    }

    #[test]
    fn test_df_with_column_appends() {
        let df = sample_df();
        let data = vec![1.0_f32, 2.0, 3.0, 4.0];
        let extended = __varg_df_with_column(&df, "rank", &data).unwrap();
        assert_eq!(__varg_df_shape(&extended).1, 4);
        assert!(__varg_df_columns(&extended).contains(&"rank".to_string()));
    }
}
