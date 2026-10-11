//! Rejeições incluem campo e linha; zero válido difere de ausência.
use crate::{IngestionError, Result};
pub fn money_field(value: Option<&str>, line: u64, field: &str) -> Result<f64> {
    let value =
        value
            .filter(|v| !v.trim().is_empty())
            .ok_or_else(|| IngestionError::InvalidField {
                line,
                field: field.into(),
                reason: "valor ausente".into(),
            })?;
    storage::Money::parse(value)
        .map(storage::Money::reais)
        .map_err(|e| IngestionError::InvalidField {
            line,
            field: field.into(),
            reason: e.to_string(),
        })
}
pub fn number_field(value: &str, line: u64, field: &str) -> Result<f64> {
    let normalized = if value.contains(',') {
        value.trim().replace('.', "").replace(',', ".")
    } else {
        value.trim().to_string()
    };
    normalized
        .parse::<f64>()
        .ok()
        .filter(|v| v.is_finite())
        .ok_or_else(|| IngestionError::InvalidField {
            line,
            field: field.into(),
            reason: "número inválido".into(),
        })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ausencia_invalido_e_zero() {
        assert_eq!(money_field(Some("0"), 2, "valor").unwrap(), 0.0);
        for val in [None, Some(""), Some("abc"), Some("NaN")] {
            let err = money_field(val, 7, "valor").unwrap_err().to_string();
            assert!(err.contains("7") && err.contains("valor"));
        }
    }
}
