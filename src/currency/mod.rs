use std::fmt;
use std::str::FromStr;

use rust_decimal::RoundingStrategy;

use core::GenericResult;
use types::{Date, Decimal};

mod cbr;
mod name_cache;
mod rate_cache;

pub mod converter;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cash {
    pub currency: &'static str,
    pub amount: Decimal,
}

impl Cash {
    pub fn new(currency: &str, amount: Decimal) -> Cash {
        Cash {
            currency: name_cache::get(currency),
            amount: amount,
        }
    }

    pub fn new_from_string(currency: &str, amount: &str) -> GenericResult<Cash> {
        Ok(Cash::new(currency, Decimal::from_str(amount).map_err(|_| format!(
            "Invalid cash amount: {:?}", amount))?))
    }

    pub fn new_from_string_positive(currency: &str, amount: &str) -> GenericResult<Cash> {
        let cash = Cash::new_from_string(currency, amount)?;

        if cash.amount <= dec!(0) {
            return Err!("Invalid cash amount: {:?}", amount);
        }

        Ok(cash)
    }
}

impl Cash {
    pub fn round(mut self) -> Cash {
        self.amount = round(self.amount);
        self
    }
}

impl fmt::Display for Cash {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{} {}", self.amount, self.currency)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct CashAssets {
    pub date: Date,
    pub cash: Cash,
}

impl CashAssets {
    #[cfg(test)]
    pub fn new(date: Date, currency: &str, amount: Decimal) -> CashAssets {
        CashAssets {date, cash: Cash::new(currency, amount)}
    }

    pub fn new_from_cash(date: Date, cash: Cash) -> CashAssets {
        CashAssets {date, cash}
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CurrencyRate {
    date: Date,
    price: Decimal,
}

pub fn round(amount: Decimal) -> Decimal {
    amount.round_dp_with_strategy(2, RoundingStrategy::RoundHalfUp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rounding() {
        assert_eq!(round(decs!("1")), decs!("1"));
        assert_eq!(round(decs!("1.1")), decs!("1.1"));
        assert_eq!(round(decs!("1.11")), decs!("1.11"));
        assert_eq!(round(decs!("1.111")), decs!("1.11"));
        assert_eq!(round(decs!("1.114")), decs!("1.11"));
        assert_eq!(round(decs!("1.124")), decs!("1.12"));
        assert_eq!(round(decs!("1.115")), decs!("1.12"));
        assert_eq!(round(decs!("1.125")), decs!("1.13"));
    }
}