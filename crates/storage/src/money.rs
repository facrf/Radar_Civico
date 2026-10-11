//! Valores monetários exatos; reais (f64) apenas na interface de compatibilidade.
use crate::{Result, StorageError};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Money(i64);
impl Money {
    pub const ZERO: Self = Self(0);
    pub fn from_cents(cents: i64) -> Self {
        Self(cents)
    }
    pub fn cents(self) -> i64 {
        self.0
    }
    pub fn reais(self) -> f64 {
        self.0 as f64 / 100.0
    }
    pub fn from_reais(value: f64) -> Result<Self> {
        if !value.is_finite() || value.abs() > 9_000_000_000_000.0 {
            return Err(StorageError::Money(
                "valor não finito ou fora do limite".into(),
            ));
        }
        Ok(Self((value * 100.0).round() as i64))
    }
    pub fn checked_add(self, other: Self) -> Result<Self> {
        self.0
            .checked_add(other.0)
            .map(Self)
            .ok_or_else(|| StorageError::Money("soma excede i64".into()))
    }
    /// Decimal brasileiro com vírgula; decimal internacional com ponto.
    /// Rejeita ausência, notação científica e mais de duas casas decimais.
    pub fn parse(value: &str) -> Result<Self> {
        let clean = value
            .trim()
            .strip_prefix("R$")
            .unwrap_or(value.trim())
            .trim();
        let (negative, clean) = match clean.strip_prefix('-') {
            Some(v) => (true, v),
            None => (false, clean.strip_prefix('+').unwrap_or(clean)),
        };
        let normalized = if clean.contains(',') {
            let parts: Vec<_> = clean.split(',').collect();
            if parts.len() != 2 {
                return Err(StorageError::Money("separadores inválidos".into()));
            }
            let groups: Vec<_> = parts[0].split('.').collect();
            if groups.len() > 1
                && (groups[0].is_empty()
                    || groups[0].len() > 3
                    || groups.iter().skip(1).any(|g| g.len() != 3))
            {
                return Err(StorageError::Money(
                    "agrupamento de milhares inválido".into(),
                ));
            }
            format!("{}.{}", parts[0].replace('.', ""), parts[1])
        } else {
            clean.to_string()
        };
        let parts: Vec<_> = normalized.split('.').collect();
        if parts.is_empty()
            || parts.len() > 2
            || parts[0].is_empty()
            || !parts[0].bytes().all(|b| b.is_ascii_digit())
        {
            return Err(StorageError::Money("valor ausente ou inválido".into()));
        }
        let fraction = parts.get(1).copied().unwrap_or("");
        if (parts.len() == 2 && fraction.is_empty())
            || fraction.len() > 2
            || !fraction.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(StorageError::Money("casas decimais inválidas".into()));
        }
        let whole: i64 = parts[0]
            .parse()
            .map_err(|_| StorageError::Money("valor fora do limite".into()))?;
        if whole > 9_000_000_000_000 {
            return Err(StorageError::Money("valor fora do limite".into()));
        }
        let frac: i64 = if fraction.is_empty() {
            0
        } else {
            fraction
                .parse()
                .map_err(|_| StorageError::Money("decimal inválido".into()))?
        };
        let cents = whole
            .checked_mul(100)
            .and_then(|v| v.checked_add(if fraction.len() == 1 { frac * 10 } else { frac }))
            .ok_or_else(|| StorageError::Money("valor fora do limite".into()))?;
        if cents > 900_000_000_000_000 {
            return Err(StorageError::Money("valor fora do limite".into()));
        }
        Ok(Self(if negative { -cents } else { cents }))
    }
    pub fn sum_reais(values: impl IntoIterator<Item = f64>) -> Result<f64> {
        values
            .into_iter()
            .try_fold(Self::ZERO, |sum, v| sum.checked_add(Self::from_reais(v)?))
            .map(Self::reais)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn decimal_exato() {
        assert_eq!(Money::parse("R$ 1.234,56").unwrap().cents(), 123456);
        assert_eq!(Money::parse("-0,10").unwrap().cents(), -10);
        assert_eq!(
            Money::parse("0.1")
                .unwrap()
                .checked_add(Money::parse("0.2").unwrap())
                .unwrap()
                .cents(),
            30
        );
        for bad in [
            "",
            "NaN",
            "inf",
            "1e3",
            "1,234",
            "1.2,34",
            "1 000",
            "1,",
            "9000000000001",
            "9000000000000,01",
        ] {
            assert!(Money::parse(bad).is_err(), "{bad}");
        }
        assert!(Money::from_reais(f64::NAN).is_err());
        assert!(Money::from_cents(i64::MAX)
            .checked_add(Money::from_cents(1))
            .is_err());
    }
}
