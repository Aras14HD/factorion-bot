use crate::math::{self, adjust_approximate_factorial, FLOAT_PRECISION};
use crate::reddit_comment::{NUMBER_DECIMALS_SCIENTIFIC, PLACEHOLDER};
use rug::ops::Pow;
use rug::{Float, Integer};
use std::fmt::Write;
use std::str::FromStr;
use std::sync::LazyLock;

// Limit for exact calculation, set to limit calculation time
pub(crate) const UPPER_CALCULATION_LIMIT: u64 = 1_000_000;
// Limit for approximation, set to ensure enough accuracy (5 decimals)
pub(crate) static UPPER_APPROXIMATION_LIMIT: LazyLock<Integer> = LazyLock::new(|| {
    Integer::from_str("1000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").unwrap()
});
// Limit for exact subfactorial calculation, set to limit calculation time
pub(crate) const UPPER_SUBFACTORIAL_LIMIT: u64 = 1_000_000;

pub(crate) static TOO_BIG_NUMBER: LazyLock<Integer> =
    LazyLock::new(|| Integer::from_str(&format!("1{}", "0".repeat(9999))).unwrap());

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum CalculatedFactorial {
    Exact(Integer),
    Approximate(Float, Integer),
    ApproximateDigits(Integer),
}

#[derive(Debug, Clone, PartialEq, Ord, Eq, Hash, PartialOrd)]
pub(crate) struct Factorial {
    pub(crate) number: Integer,
    pub(crate) levels: Vec<i32>,
    pub(crate) factorial: CalculatedFactorial,
}

impl Ord for CalculatedFactorial {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        match (self, other) {
            (Self::Exact(this), Self::Exact(other)) => this.cmp(other),
            (Self::Exact(_), _) => std::cmp::Ordering::Greater,
            (Self::Approximate(this_base, this_exp), Self::Approximate(other_base, other_exp)) => {
                let exp_ord = this_exp.cmp(other_exp);
                let std::cmp::Ordering::Equal = exp_ord else {
                    return exp_ord;
                };
                this_base.total_cmp(other_base)
            }
            (Self::Approximate(_, _), _) => std::cmp::Ordering::Greater,
            (Self::ApproximateDigits(this), Self::ApproximateDigits(other)) => this.cmp(other),
            (Self::ApproximateDigits(_), _) => std::cmp::Ordering::Less,
        }
    }
}

impl PartialOrd for CalculatedFactorial {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for CalculatedFactorial {}

impl std::hash::Hash for CalculatedFactorial {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        match self {
            Self::Exact(factorial) => {
                state.write_u8(1);
                factorial.hash(state);
            }
            Self::Approximate(base, exponent) => {
                state.write_u8(2);
                let raw_base = base.clone().into_raw();
                raw_base.prec.hash(state);
                raw_base.sign.hash(state);
                raw_base.exp.hash(state);
                raw_base.d.hash(state);
                exponent.hash(state);
            }
            Self::ApproximateDigits(digits) => {
                state.write_u8(3);
                digits.hash(state);
            }
        }
    }
}

impl Factorial {
    pub(crate) fn format(
        &self,
        acc: &mut String,
        force_shorten: bool,
    ) -> Result<(), std::fmt::Error> {
        let factorial_string = self.levels.iter().rev().fold(String::new(), |a, e| {
            format!(
                "{}{}{}",
                a,
                Self::get_factorial_level_string(*e),
                PLACEHOLDER
            )
        });
        match &self.factorial {
            CalculatedFactorial::Exact(factorial) => {
                let factorial = if self.is_too_long() || force_shorten {
                    Self::truncate(factorial, true)
                } else {
                    factorial.to_string()
                };
                write!(
                    acc,
                    "{}{} is {} \n\n",
                    factorial_string, self.number, factorial
                )
            }
            CalculatedFactorial::Approximate(base, exponent) => {
                let (base, exponent) =
                    adjust_approximate_factorial((base.clone(), exponent.clone()));
                let exponent = if self.is_too_long() || force_shorten {
                    format!("({})", Self::truncate(&exponent, false))
                } else {
                    exponent.to_string()
                };
                let number = if self.number > *TOO_BIG_NUMBER || force_shorten {
                    Self::truncate(&self.number, false)
                } else {
                    self.number.to_string()
                };
                let base = base.to_f64();
                write!(
                    acc,
                    "{}{} is approximately {} × 10^{} \n\n",
                    factorial_string, number, base, exponent
                )
            }
            CalculatedFactorial::ApproximateDigits(digits) => {
                let digits = if self.is_too_long() || force_shorten {
                    Self::truncate(digits, false)
                } else {
                    digits.to_string()
                };
                let number = if self.number > *TOO_BIG_NUMBER || force_shorten {
                    Self::truncate(&self.number, false)
                } else {
                    self.number.to_string()
                };
                write!(
                    acc,
                    "{}{} has approximately {} digits \n\n",
                    factorial_string, number, digits
                )
            }
        }
    }

    fn truncate(number: &Integer, add_roughly: bool) -> String {
        let length = (Float::with_val(FLOAT_PRECISION, number).ln() / &*math::LN10)
            .to_integer_round(rug::float::Round::Down)
            .unwrap()
            .0;
        let truncated_number: Integer = number
            / (Float::with_val(FLOAT_PRECISION, 10)
                .pow((length.clone() - NUMBER_DECIMALS_SCIENTIFIC - 1u8).max(Integer::ZERO))
                .to_integer()
                .unwrap());
        let mut truncated_number = truncated_number.to_string();
        if truncated_number.len() > NUMBER_DECIMALS_SCIENTIFIC {
            math::round(&mut truncated_number);
        }
        if let Some(mut digit) = truncated_number.pop() {
            while digit == '0' {
                digit = match truncated_number.pop() {
                    Some(x) => x,
                    None => break,
                }
            }
            truncated_number.push(digit);
        }
        // Only add decimal if we have more than one digit
        if truncated_number.len() > 1 {
            truncated_number.insert(1, '.'); // Decimal point
        }
        if length > NUMBER_DECIMALS_SCIENTIFIC + 1 {
            format!(
                "{}{} × 10^{}",
                if add_roughly { "roughly " } else { "" },
                truncated_number,
                length
            )
        } else {
            number.to_string()
        }
    }

    pub(crate) fn is_aproximate_digits(&self) -> bool {
        matches!(self.factorial, CalculatedFactorial::ApproximateDigits(_))
    }
    pub(crate) fn is_approximate(&self) -> bool {
        matches!(self.factorial, CalculatedFactorial::Approximate(_, _))
    }
    pub(crate) fn is_too_long(&self) -> bool {
        let n = match &self.factorial {
            CalculatedFactorial::Exact(n)
            | CalculatedFactorial::ApproximateDigits(n)
            | CalculatedFactorial::Approximate(_, n) => n,
        };
        n > &*TOO_BIG_NUMBER
    }

    pub(crate) fn get_factorial_level_string(level: i32) -> &'static str {
        let prefix = match level {
            -1 => "Sub",
            1 => "The ",
            2 => "Double-",
            3 => "Triple-",
            4 => "Quadruple-",
            5 => "Quintuple-",
            6 => "Sextuple-",
            7 => "Septuple-",
            8 => "Octuple-",
            9 => "Nonuple-",
            10 => "Decuple-",
            11 => "Undecuple-",
            12 => "Duodecuple-",
            13 => "Tredecuple-",
            14 => "Quattuordecuple-",
            15 => "Quindecuple-",
            16 => "Sexdecuple-",
            17 => "Septendecuple-",
            18 => "Octodecuple-",
            19 => "Novemdecuple-",
            20 => "Vigintuple-",
            21 => "Unvigintuple-",
            22 => "Duovigintuple-",
            23 => "Trevigintuple-",
            24 => "Quattuorvigintuple-",
            25 => "Quinvigintuple-",
            26 => "Sexvigintuple-",
            27 => "Septenvigintuple-",
            28 => "Octovigintuple-",
            29 => "Novemvigintuple-",
            30 => "Trigintuple-",
            31 => "Untrigintuple-",
            32 => "Duotrigintuple-",
            33 => "Tretrigintuple-",
            34 => "Quattuortrigintuple-",
            35 => "Quintrigintuple-",
            36 => "Sextrigintuple-",
            37 => "Septentrigintuple-",
            38 => "Octotrigintuple-",
            39 => "Novemtrigintuple-",
            40 => "Quadragintuple-",
            41 => "Unquadragintuple-",
            42 => "Duoquadragintuple-",
            43 => "Trequadragintuple-",
            44 => "Quattuorquadragintuple-",
            45 => "Quinquadragintuple-",
            _ => {
                let mut suffix = String::new();
                write!(&mut suffix, "{}-", level).unwrap();
                Box::leak(suffix.into_boxed_str())
            }
        };

        prefix
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use math::FLOAT_PRECISION;
    use rug::Integer;

    #[test]
    fn test_factorial_level_string() {
        assert_eq!(Factorial::get_factorial_level_string(1), "The ");
        assert_eq!(Factorial::get_factorial_level_string(2), "Double-");
        assert_eq!(Factorial::get_factorial_level_string(3), "Triple-");
        assert_eq!(
            Factorial::get_factorial_level_string(45),
            "Quinquadragintuple-"
        );
        assert_eq!(Factorial::get_factorial_level_string(50), "50-");
    }

    #[test]
    fn test_factorial_format() {
        let mut acc = String::new();
        let factorial = Factorial {
            number: 5.into(),
            levels: vec![1],
            factorial: CalculatedFactorial::Exact(Integer::from(120)),
        };
        factorial.format(&mut acc, false).unwrap();
        assert_eq!(acc, "The factorial of 5 is 120 \n\n");

        let mut acc = String::new();
        let factorial = Factorial {
            number: 5.into(),
            levels: vec![-1],
            factorial: CalculatedFactorial::Exact(Integer::from(120)),
        };
        factorial.format(&mut acc, false).unwrap();
        assert_eq!(acc, "Subfactorial of 5 is 120 \n\n");

        let mut acc = String::new();
        let factorial = Factorial {
            number: 5.into(),
            levels: vec![1],
            factorial: CalculatedFactorial::Approximate(
                Float::with_val(FLOAT_PRECISION, 120),
                3.into(),
            ),
        };
        factorial.format(&mut acc, false).unwrap();
        assert_eq!(acc, "The factorial of 5 is approximately 1.2 × 10^5 \n\n");

        let mut acc = String::new();
        let factorial = Factorial {
            number: 5.into(),
            levels: vec![1],
            factorial: CalculatedFactorial::ApproximateDigits(3.into()),
        };
        factorial.format(&mut acc, false).unwrap();
        assert_eq!(acc, "The factorial of 5 has approximately 3 digits \n\n");

        let mut acc = String::new();
        let factorial = Factorial {
            number: 5.into(),
            levels: vec![1],
            factorial: CalculatedFactorial::Exact(Integer::from(120)),
        };
        factorial.format(&mut acc, true).unwrap();
        assert_eq!(acc, "The factorial of 5 is 120 \n\n");
    }
}
