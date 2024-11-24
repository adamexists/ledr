/* Copyright © 2024 Adam House <adam@adamexists.com>
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU General Public License as published by
 * the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE. See the
 * GNU General Public License for more details.
 *
 * You should have received a copy of the GNU General Public License
 * along with this program. If not, see <https://www.gnu.org/licenses/>.
 */
use anyhow::{bail, Error};
use std::cmp::Ordering;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::iter::Sum;
use std::ops::{
	Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign,
};

/// A general-purpose rational number backed by a fraction of u128s. It is
/// precise for all numbers that can be reflected in that format, which vastly
/// exceeds the requirements of any human accounting. The reason it was
/// designed so rigorously is primarily due to exchange rate calculations in
/// potentially long chains of exchange rates, which is of interest to, for
/// example, traders of cryptocurrency or complex foreign exchange use cases.
///
/// Automatically simplifies its underlying fractional representation.
#[derive(Clone, Copy, Debug, Default)]
pub struct Quant {
	numerator: u128,
	denominator: u128,

	/// Is always zero if the numerator is zero, else is intuitive.
	is_negative: bool,

	/// How many decimal places to render when asked to print. Will round with
	/// banker's rounding when underlying precision exceeds what is requested.
	///
	/// Has no effect on the underlying fraction.
	render_precision: u32,
}

impl Quant {
	pub fn zero() -> Self {
		Self {
			numerator: 0,
			denominator: 1,
			render_precision: 0,
			is_negative: false,
		}
	}

	// TODO: This format of new() method is carried over from a prior Quant
	//  implementation, and it should be overhauled at some point.
	pub fn new(number: i128, resolution: u32) -> Self {
		let mut out = Self {
			numerator: number.unsigned_abs(),
			denominator: 10u128.pow(resolution),
			render_precision: resolution,
			is_negative: number < 0,
		};
		out.reduce();
		out
	}

	pub fn from_frac(numerator: i128, denominator: i128) -> Self {
		if denominator == 0 {
			panic!("Denominator cannot be zero");
		}

		let mut out = Self {
			numerator: numerator.unsigned_abs(),
			denominator: denominator.unsigned_abs(),
			render_precision: 0,
			is_negative: (numerator < 0) ^ (denominator < 0),
		};

		out.reduce();
		out
	}

	pub fn from_i128(amount: i128) -> Self {
		Self {
			numerator: amount.unsigned_abs(),
			denominator: 1,
			render_precision: 0,
			is_negative: amount < 0,
		}
	}

	pub fn from_str(input: &str) -> Result<Self, Error> {
		// Check for negative sign explicitly and removing it for parsing
		let is_negative = input.starts_with('-');
		let sanitized = input.trim_start_matches('-');

		let parts: Vec<&str> = sanitized.split('.').collect();
		let mut precision = 0u32;

		let (numerator, denominator) = match parts.len() {
			1 => (parts[0].parse::<u128>()?, 1),
			2 => {
				let whole = parts[0].parse::<u128>()?;
				let decimal = parts[1];
				precision = decimal.len() as u32;
				let scale = 10u128.pow(precision);
				let fractional = decimal.parse::<u128>()?;
				let numerator = whole * scale + fractional;
				(numerator, scale)
			},
			_ => bail!("Invalid decimal format"),
		};

		let mut out = Self {
			numerator,
			denominator,
			render_precision: precision,
			is_negative: is_negative && numerator > 0,
		};
		out.reduce();
		Ok(out)
	}

	/// Modifies the underlying fraction to represent a value that is rounded
	/// off to the given number of decimal places when rendered as a decimal.
	/// Uses Banker's rounding (rounds to nearest, ties to even).
	pub fn round(&mut self, resolution: u32) {
		let scale = 10u128.pow(resolution);
		let scaled_numerator = self.numerator * scale;
		let quotient = scaled_numerator / self.denominator;
		let remainder = scaled_numerator % self.denominator;

		// Perform Banker's rounding
		let half_denom = (self.denominator + 1) / 2;
		let rounded_quotient = if remainder > half_denom
			|| (remainder == half_denom && quotient % 2 != 0)
		{
			quotient + 1
		} else {
			quotient
		};

		self.numerator = rounded_quotient;
		self.denominator = scale;
		self.render_precision = resolution;
		self.is_negative = self.is_negative && rounded_quotient > 0;

		self.reduce();
	}

	pub fn resolution(&self) -> u32 {
		self.render_precision
	}

	pub fn set_render_precision(&mut self, precision: u32) {
		self.render_precision = precision;
	}

	pub fn abs(&self) -> Self {
		Self {
			is_negative: false,
			..*self
		}
	}

	pub fn negate(&mut self) {
		self.is_negative = !self.is_negative;
	}

	/// Reduces the underlying fraction as much as possible while still
	/// representing the same value. Has no user-visible effect; we call this
	/// after every operation that effects the fraction, to guard against
	/// overflow when dealing with high-precision values.
	fn reduce(&mut self) {
		let gcd = Self::gcd(self.numerator, self.denominator);
		self.numerator /= gcd;
		self.denominator /= gcd;
	}

	/// Implementation of Euclid's algorithm for greatest common divisor
	fn gcd(mut a: u128, mut b: u128) -> u128 {
		while b != 0 {
			let temp = b;
			b = a % b;
			a = temp;
		}
		a
	}
}

impl fmt::Display for Quant {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let mut numerator = self.numerator;
		let denominator = self.denominator;

		let integer_part = numerator / denominator;
		numerator %= denominator;

		let mut fraction_str = String::new();
		let mut remainder = numerator;
		let precision = f.precision().unwrap_or(self.render_precision as usize);
		for _ in 0..precision {
			remainder *= 10;
			let digit = remainder / denominator;
			remainder %= denominator;
			fraction_str.push(std::char::from_digit(digit as u32, 10).unwrap());
			if remainder == 0 {
				break;
			}
		}

		if fraction_str.len() < self.render_precision as usize {
			let zeros_to_add =
				self.render_precision as usize - fraction_str.len();
			fraction_str.push_str(&"0".repeat(zeros_to_add));
		}

		while fraction_str.ends_with('0')
			&& fraction_str.len() > self.render_precision as usize
		{
			fraction_str.pop();
		}

		let mut int_str = integer_part.to_string();
		let mut i = int_str.len() as isize - 3;
		while i > 0 {
			int_str.insert(i as usize, ',');
			i -= 3;
		}

		let formatted = if fraction_str.is_empty() {
			int_str
		} else {
			format!("{}.{}", int_str, fraction_str)
		};

		if self.is_negative {
			write!(f, "-{}", formatted)
		} else {
			write!(f, "{}", formatted)
		}
	}
}

// -----------------
// -- BOILERPLATE --
// -----------------

impl Add for Quant {
	type Output = Self;

	fn add(self, rhs: Self) -> Self::Output {
		// Calculate the resulting numerator and denominator
		let term_a = self.numerator * rhs.denominator;
		let term_b = rhs.numerator * self.denominator;

		let (numerator, result_is_negative) =
			match (self.is_negative, rhs.is_negative) {
				(true, true) => (term_a + term_b, true),
				(false, false) => (term_a + term_b, false),
				(true, false) => {
					if term_a > term_b {
						(term_a - term_b, true)
					} else {
						(term_b - term_a, false)
					}
				},
				(false, true) => {
					if term_a > term_b {
						(term_a - term_b, false)
					} else {
						(term_b - term_a, true)
					}
				},
			};

		let mut out = Self {
			numerator,
			denominator: self.denominator * rhs.denominator,
			render_precision: self.render_precision.max(rhs.render_precision),
			is_negative: result_is_negative && numerator > 0,
		};
		out.reduce();
		out
	}
}

impl AddAssign for Quant {
	fn add_assign(&mut self, rhs: Self) {
		*self = *self + rhs;
	}
}

impl Sum for Quant {
	fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
		iter.fold(Quant::zero(), |acc, scalar| acc + scalar)
	}
}

impl Sub for Quant {
	type Output = Self;

	fn sub(self, rhs: Self) -> Self::Output {
		// Calculate the resulting numerator and denominator
		let term_a = self.numerator * rhs.denominator;
		let term_b = rhs.numerator * self.denominator;

		let (numerator, result_is_negative) =
			match (self.is_negative, !rhs.is_negative) {
				(true, true) => (term_a + term_b, true),
				(false, false) => (term_a + term_b, false),
				(true, false) => {
					if term_a > term_b {
						(term_a - term_b, true)
					} else {
						(term_b - term_a, false)
					}
				},
				(false, true) => {
					if term_a > term_b {
						(term_a - term_b, false)
					} else {
						(term_b - term_a, true)
					}
				},
			};

		let denominator = self.denominator * rhs.denominator;

		let mut out = Self {
			numerator,
			denominator,
			render_precision: self.render_precision.max(rhs.render_precision),
			is_negative: result_is_negative && numerator > 0,
		};
		out.reduce();
		out
	}
}

impl SubAssign for Quant {
	fn sub_assign(&mut self, rhs: Self) {
		*self = *self - rhs;
	}
}

impl Mul for Quant {
	type Output = Self;

	fn mul(self, rhs: Self) -> Self::Output {
		let numerator = self.numerator * rhs.numerator;
		let denominator = self.denominator * rhs.denominator;
		let is_negative =
			numerator != 0 && (self.is_negative ^ rhs.is_negative);

		let mut out = Self {
			numerator,
			denominator,
			is_negative,
			render_precision: self.render_precision.max(rhs.render_precision),
		};
		out.reduce();
		out
	}
}

impl MulAssign for Quant {
	fn mul_assign(&mut self, rhs: Self) {
		*self = *self * rhs;
	}
}

impl Mul<i128> for Quant {
	type Output = Self;

	fn mul(self, rhs: i128) -> Self::Output {
		let is_rhs_negative = rhs < 0;
		let abs_rhs = rhs.unsigned_abs();

		let numerator = self.numerator * abs_rhs;
		let is_negative =
			numerator != 0 && (self.is_negative ^ is_rhs_negative);

		let mut out = Self {
			numerator,
			denominator: self.denominator,
			is_negative,
			render_precision: self.render_precision,
		};
		out.reduce();
		out
	}
}

impl Mul<Quant> for i128 {
	type Output = Quant;

	fn mul(self, rhs: Quant) -> Self::Output {
		let a = Quant::from_i128(self);
		a * rhs
	}
}

impl Div for Quant {
	type Output = Self;

	fn div(self, rhs: Self) -> Self::Output {
		if rhs.numerator == 0 {
			panic!("Attempt to divide by zero");
		}

		let numerator = self.numerator * rhs.denominator;
		let denominator = self.denominator * rhs.numerator;
		let is_negative = numerator != 0 && self.is_negative ^ rhs.is_negative;

		let mut out = Self {
			numerator,
			denominator,
			is_negative,
			render_precision: self.render_precision.max(rhs.render_precision),
		};
		out.reduce();
		out
	}
}

impl DivAssign for Quant {
	fn div_assign(&mut self, rhs: Self) {
		*self = *self / rhs;
	}
}

impl Div<i128> for Quant {
	type Output = Self;

	fn div(self, rhs: i128) -> Self::Output {
		let a = Quant::from_i128(rhs);
		self / a
	}
}

impl Div<Quant> for i128 {
	type Output = Quant;

	fn div(self, rhs: Quant) -> Self::Output {
		let a = Quant::from_i128(self);
		rhs / a
	}
}

impl Neg for Quant {
	type Output = Self;

	fn neg(self) -> Self::Output {
		Self {
			is_negative: !self.is_negative,
			..self
		}
	}
}

impl PartialEq<i128> for Quant {
	fn eq(&self, &other: &i128) -> bool {
		let is_other_negative = other < 0;
		let abs_other = other.unsigned_abs();

		self.is_negative == is_other_negative
			&& self.numerator == abs_other * self.denominator
	}
}

impl PartialEq for Quant {
	fn eq(&self, other: &Self) -> bool {
		self.numerator * other.denominator == other.numerator * self.denominator
			&& self.is_negative == other.is_negative
	}
}

impl PartialEq<Quant> for i128 {
	fn eq(&self, other: &Quant) -> bool {
		if (*self < 0) ^ other.is_negative {
			return false;
		};

		*self as u128 * other.denominator == other.numerator
	}
}

impl Eq for Quant {}

impl PartialOrd for Quant {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl PartialOrd<i128> for Quant {
	fn partial_cmp(&self, other: &i128) -> Option<Ordering> {
		if self.numerator == 0 && *other == 0 {
			return Some(Ordering::Equal);
		}

		let self_positive = !self.is_negative;
		let other_positive = *other >= 0;

		match (self_positive, other_positive) {
			(true, false) => Some(Ordering::Greater),
			(false, true) => Some(Ordering::Less),
			_ => {
				let other_abs = (*other).unsigned_abs();
				let scaled_other = other_abs * self.denominator;

				if other_positive {
					scaled_other
						.partial_cmp(&self.numerator)
						.map(Ordering::reverse)
				} else {
					self.numerator
						.partial_cmp(&scaled_other)
						.map(Ordering::reverse)
				}
			},
		}
	}
}

impl PartialOrd<Quant> for i128 {
	fn partial_cmp(&self, other: &Quant) -> Option<Ordering> {
		if *self == 0 && other.numerator == 0 {
			return Some(Ordering::Equal);
		}

		let self_positive = *self >= 0;
		let other_positive = !other.is_negative;

		match (self_positive, other_positive) {
			(true, false) => Some(Ordering::Greater),
			(false, true) => Some(Ordering::Less),
			_ => {
				let self_abs = self.unsigned_abs();
				let scaled_self = self_abs * other.denominator;

				if self_positive {
					scaled_self.partial_cmp(&other.numerator)
				} else {
					other.numerator.partial_cmp(&scaled_self)
				}
			},
		}
	}
}

impl Ord for Quant {
	fn cmp(&self, other: &Self) -> Ordering {
		if self.numerator == 0 && other.numerator == 0 {
			return Ordering::Equal;
		}

		match (self.is_negative, other.is_negative) {
			(false, true) => Ordering::Greater,
			(true, false) => Ordering::Less,
			_ => {
				let scaled_self = self.numerator * other.denominator;
				let scaled_other = other.numerator * self.denominator;

				if self.is_negative {
					scaled_other.cmp(&scaled_self)
				} else {
					scaled_self.cmp(&scaled_other)
				}
			},
		}
	}
}

impl Hash for Quant {
	fn hash<H: Hasher>(&self, state: &mut H) {
		self.numerator.hash(state);
		self.denominator.hash(state);
		self.is_negative.hash(state);
		// `render_precision` intentionally excluded from the hash.
	}
}

// TODO: This is many tests and that's great, but check for redundancies.
#[cfg(test)]
mod tests {
	use super::*;

	mod creation {
		use super::*;

		mod new {
			use super::*;

			#[test]
			fn test_positive_number_with_precision() {
				let scalar = Quant::new(123, 2);
				assert_eq!(scalar.numerator, 123);
				assert_eq!(scalar.denominator, 100);
				assert_eq!(scalar.render_precision, 2);
			}

			#[test]
			fn test_zero_number() {
				let scalar = Quant::new(0, 5);
				assert_eq!(scalar.numerator, 0);
				assert_eq!(scalar.denominator, 1);
				assert_eq!(scalar.render_precision, 5);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_negative_number_with_precision() {
				let scalar = Quant::new(-456, 3);
				assert_eq!(scalar.numerator, 57);
				assert_eq!(scalar.denominator, 125);
				assert_eq!(scalar.render_precision, 3);
				assert!(scalar.is_negative);
			}

			#[test]
			fn test_high_precision() {
				let scalar = Quant::new(789, 10);
				assert_eq!(scalar.numerator, 789);
				assert_eq!(scalar.denominator, 10u128.pow(10));
				assert_eq!(scalar.render_precision, 10);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_no_precision() {
				let scalar = Quant::new(42, 0);
				assert_eq!(scalar.numerator, 42);
				assert_eq!(scalar.denominator, 1);
				assert_eq!(scalar.render_precision, 0);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_large_number_and_precision() {
				let scalar = Quant::new(987654321, 15);
				assert_eq!(scalar.numerator, 987654321);
				assert_eq!(scalar.denominator, 10u128.pow(15));
				assert_eq!(scalar.render_precision, 15);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_number_reduction() {
				let scalar = Quant::new(200, 2);
				assert_eq!(scalar.numerator, 2);
				assert_eq!(scalar.denominator, 1);
				assert_eq!(scalar.render_precision, 2);
			}

			#[test]
			fn test_negative_number_reduction() {
				let scalar = Quant::new(-200, 2);
				assert_eq!(scalar.numerator, 2);
				assert_eq!(scalar.denominator, 1);
				assert_eq!(scalar.render_precision, 2);
				assert!(scalar.is_negative);
			}

			#[test]
			fn test_zero_precision_with_large_number() {
				let scalar = Quant::new(999999999999999999, 0);
				assert_eq!(scalar.numerator, 999999999999999999);
				assert_eq!(scalar.denominator, 1);
				assert_eq!(scalar.render_precision, 0);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_large_negative_number_with_high_precision() {
				let scalar = Quant::new(-123456789, 18);
				assert_eq!(scalar.numerator, 123456789);
				assert_eq!(scalar.denominator, 10u128.pow(18));
				assert_eq!(scalar.render_precision, 18);
				assert!(scalar.is_negative);
			}
		}

		mod from_frac {
			use super::*;

			#[test]
			#[should_panic(expected = "Denominator cannot be zero")]
			fn test_zero_denominator() {
				Quant::from_frac(1, 0);
			}

			#[test]
			fn test_positive_fraction() {
				let scalar = Quant::from_frac(6, 8);
				assert_eq!(scalar.numerator, 3);
				assert_eq!(scalar.denominator, 4);
				assert_eq!(scalar.render_precision, 0);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_negative_fraction_numerator() {
				let scalar = Quant::from_frac(-6, 8);
				assert_eq!(scalar.numerator, 3);
				assert_eq!(scalar.denominator, 4);
				assert_eq!(scalar.render_precision, 0);
				assert!(scalar.is_negative);
			}

			#[test]
			fn test_negative_fraction_denominator() {
				let scalar = Quant::from_frac(6, -8);
				assert_eq!(scalar.numerator, 3);
				assert_eq!(scalar.denominator, 4);
				assert_eq!(scalar.render_precision, 0);
				assert!(scalar.is_negative);
			}

			#[test]
			fn test_negative_fraction_both() {
				let scalar = Quant::from_frac(-6, -8);
				assert_eq!(scalar.numerator, 3);
				assert_eq!(scalar.denominator, 4);
				assert_eq!(scalar.render_precision, 0);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_reduction_to_lowest_terms() {
				let scalar = Quant::from_frac(100, 400);
				assert_eq!(scalar.numerator, 1);
				assert_eq!(scalar.denominator, 4);
				assert_eq!(scalar.render_precision, 0);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_large_numbers() {
				let scalar =
					Quant::from_frac(12345678901234567890, 9876543210987654321);
				assert_eq!(scalar.numerator, 137174210);
				assert_eq!(scalar.denominator, 109739369);
				assert_eq!(scalar.render_precision, 0);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_one_as_denominator() {
				let scalar = Quant::from_frac(7, 1);
				assert_eq!(scalar.numerator, 7);
				assert_eq!(scalar.denominator, 1);
				assert_eq!(scalar.render_precision, 0);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_zero_as_numerator() {
				let scalar = Quant::from_frac(0, 5);
				assert_eq!(scalar.numerator, 0);
				assert_eq!(scalar.denominator, 1);
				assert_eq!(scalar.render_precision, 0);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_already_reduced_fraction() {
				let scalar = Quant::from_frac(3, 4);
				assert_eq!(scalar.numerator, 3);
				assert_eq!(scalar.denominator, 4);
				assert_eq!(scalar.render_precision, 0);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_negative_one_as_denominator() {
				let scalar = Quant::from_frac(7, -1);
				assert_eq!(scalar.numerator, 7);
				assert_eq!(scalar.denominator, 1);
				assert_eq!(scalar.render_precision, 0);
				assert!(scalar.is_negative);
			}

			#[test]
			fn test_negative_one_as_numerator() {
				let scalar = Quant::from_frac(-1, 3);
				assert_eq!(scalar.numerator, 1);
				assert_eq!(scalar.denominator, 3);
				assert_eq!(scalar.render_precision, 0);
				assert!(scalar.is_negative);
			}

			#[test]
			fn test_minimal_fraction() {
				let scalar = Quant::from_frac(1, 2);
				assert_eq!(scalar.numerator, 1);
				assert_eq!(scalar.denominator, 2);
				assert_eq!(scalar.render_precision, 0);
				assert!(!scalar.is_negative);
			}
		}

		mod from_str {
			use super::*;

			#[test]
			fn test_from_str_positive_integer() {
				let scalar = Quant::from_str("123").unwrap();
				assert_eq!(scalar.numerator, 123);
				assert_eq!(scalar.denominator, 1);
				assert_eq!(scalar.render_precision, 0);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_from_str_negative_integer() {
				let scalar = Quant::from_str("-123").unwrap();
				assert_eq!(scalar.numerator, 123);
				assert_eq!(scalar.denominator, 1);
				assert_eq!(scalar.render_precision, 0);
				assert!(scalar.is_negative);
			}

			#[test]
			fn test_from_str_positive_decimal() {
				let scalar = Quant::from_str("123.456").unwrap();
				assert_eq!(scalar.numerator, 15432);
				assert_eq!(scalar.denominator, 125);
				assert_eq!(scalar.render_precision, 3);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_from_str_negative_decimal() {
				let scalar = Quant::from_str("-123.456").unwrap();
				assert_eq!(scalar.numerator, 15432);
				assert_eq!(scalar.denominator, 125);
				assert_eq!(scalar.render_precision, 3);
				assert!(scalar.is_negative);
			}

			#[test]
			fn test_from_str_invalid_format() {
				let result = Quant::from_str("123.45.67");
				assert!(
					result.is_err(),
					"Expected error for invalid decimal format"
				);
			}

			#[test]
			fn test_from_str_invalid_characters() {
				let result = Quant::from_str("abc123");
				assert!(
					result.is_err(),
					"Expected error for invalid characters"
				);
			}

			#[test]
			fn test_from_str_empty_string() {
				let result = Quant::from_str("");
				assert!(result.is_err(), "Expected error for empty string");
			}

			#[test]
			fn test_from_str_zero() {
				let scalar = Quant::from_str("0").unwrap();
				assert_eq!(scalar.numerator, 0);
				assert_eq!(scalar.denominator, 1);
				assert_eq!(scalar.render_precision, 0);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_from_str_negative_zero() {
				let scalar = Quant::from_str("-0").unwrap();
				assert_eq!(scalar.numerator, 0);
				assert_eq!(scalar.denominator, 1);
				assert_eq!(scalar.render_precision, 0);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_from_str_zero_decimal() {
				let scalar = Quant::from_str("0.00").unwrap();
				assert_eq!(scalar.numerator, 0);
				assert_eq!(scalar.denominator, 1);
				assert_eq!(scalar.render_precision, 2);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_from_str_negative_zero_decimal() {
				let scalar = Quant::from_str("-0.00").unwrap();
				assert_eq!(scalar.numerator, 0);
				assert_eq!(scalar.denominator, 1);
				assert_eq!(scalar.render_precision, 2);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_from_str_near_zero_decimal() {
				let scalar = Quant::from_str("0.05").unwrap();
				assert_eq!(scalar.numerator, 1);
				assert_eq!(scalar.denominator, 20);
				assert_eq!(scalar.render_precision, 2);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_from_str_negative_near_zero_decimal() {
				let scalar = Quant::from_str("-0.05").unwrap();
				assert_eq!(scalar.numerator, 1);
				assert_eq!(scalar.denominator, 20);
				assert_eq!(scalar.render_precision, 2);
				assert!(scalar.is_negative);
			}
		}

		mod from_i128 {
			use super::*;

			#[test]
			fn test_from_i128_positive() {
				let scalar = Quant::from_i128(42);
				assert_eq!(scalar.numerator, 42);
				assert_eq!(scalar.denominator, 1);
				assert_eq!(scalar.render_precision, 0);
				assert!(!scalar.is_negative);
			}

			#[test]
			fn test_from_i128_negative() {
				let scalar = Quant::from_i128(-42);
				assert_eq!(scalar.numerator, 42);
				assert_eq!(scalar.denominator, 1);
				assert_eq!(scalar.render_precision, 0);
				assert!(scalar.is_negative);
			}
		}
	}

	mod math {
		use super::*;

		mod add {
			use super::*;

			#[test]
			fn test_add() {
				let a = Quant::from_frac(1, 2);
				let b = Quant::from_frac(1, 3);
				assert_eq!(a + b, Quant::from_frac(5, 6));
			}

			#[test]
			fn test_add_with_integer() {
				let a = Quant::from_frac(1, 2);
				let b = Quant::from_i128(2);
				assert_eq!(a + b, Quant::from_frac(5, 2));
			}

			#[test]
			fn test_add_large_numbers() {
				let a = Quant::from_frac(123456789, 987654321);
				let b = Quant::from_frac(987654321, 123456789);
				assert_eq!(
					a + b,
					Quant::from_frac(990702636540161562, 121932631112635269)
				);
			}

			#[test]
			fn test_add_negative_numbers() {
				let a = Quant::from_frac(-1, 3);
				let b = Quant::from_frac(-2, 5);
				assert_eq!(a + b, Quant::from_frac(-11, 15));
			}

			#[test]
			fn test_add_mixed_signs() {
				let a = Quant::from_frac(5, 6);
				let b = Quant::from_frac(-1, 3);
				assert_eq!(a + b, Quant::from_frac(1, 2));
			}

			#[test]
			fn test_add_small_numbers() {
				let a = Quant::from_frac(1, 1000000);
				let b = Quant::from_frac(1, 1000000);
				assert_eq!(a + b, Quant::from_frac(1, 500000));
			}
		}

		mod add_assign {
			use super::*;

			#[test]
			fn test_add_assign() {
				let mut a = Quant::from_frac(1, 2);
				let b = Quant::from_frac(1, 3);
				a += b;
				assert_eq!(a, Quant::from_frac(5, 6));
			}

			#[test]
			fn test_add_assign_large_numbers() {
				let mut a = Quant::from_frac(123456789, 987654321);
				let b = Quant::from_frac(987654321, 123456789);
				a += b;
				assert_eq!(
					a,
					Quant::from_frac(990702636540161562, 121932631112635269)
				);
			}

			#[test]
			fn test_add_assign_negative_numbers() {
				let mut a = Quant::from_frac(-1, 3);
				let b = Quant::from_frac(-2, 5);
				a += b;
				assert_eq!(a, Quant::from_frac(-11, 15));
			}

			#[test]
			fn test_add_assign_mixed_signs() {
				let mut a = Quant::from_frac(-1, 3);
				let b = Quant::from_frac(2, 5);
				a += b;
				assert_eq!(a, Quant::from_frac(1, 15));
			}
		}

		mod sub {
			use super::*;

			#[test]
			fn test_sub() {
				let a = Quant::from_frac(3, 4);
				let b = Quant::from_frac(1, 4);
				assert_eq!(a - b, Quant::from_frac(2, 4));
			}

			#[test]
			fn test_sub_with_integer() {
				let a = Quant::from_i128(5);
				let b = Quant::from_frac(1, 2);
				assert_eq!(a - b, Quant::from_frac(9, 2));
			}

			#[test]
			fn test_sub_negative_numbers() {
				let a = Quant::from_frac(-1, 2);
				let b = Quant::from_frac(-1, 3);
				assert_eq!(a - b, Quant::from_frac(-1, 6));
			}

			#[test]
			fn test_sub_mixed_signs() {
				let a = Quant::from_frac(5, 6);
				let b = Quant::from_frac(-1, 3);
				assert_eq!(a - b, Quant::from_frac(7, 6));

				let a = Quant::from_frac(-5, 6);
				let b = Quant::from_frac(1, 3);
				assert_eq!(a - b, Quant::from_frac(-7, 6));
			}

			#[test]
			fn test_sub_small_numbers() {
				let a = Quant::from_frac(1, 1000000);
				let b = Quant::from_frac(1, 1000000);
				assert_eq!(a - b, Quant::from_frac(0, 1));
			}
		}

		mod sub_assign {
			use super::*;

			#[test]
			fn test_sub_assign() {
				let mut a = Quant::from_frac(3, 4);
				let b = Quant::from_frac(1, 4);
				a -= b;
				assert_eq!(a, Quant::from_frac(2, 4));
			}

			#[test]
			fn test_sub_assign_negative_numbers() {
				let mut a = Quant::from_frac(-1, 2);
				let b = Quant::from_frac(-1, 3);
				a -= b;
				assert_eq!(a, Quant::from_frac(-1, 6));
			}

			#[test]
			fn test_sub_assign_mixed_signs() {
				let mut a = Quant::from_frac(5, 6);
				let b = Quant::from_frac(-1, 3);
				a -= b;
				assert_eq!(a, Quant::from_frac(7, 6));
			}
		}

		mod mul {
			use super::*;

			#[test]
			fn test_mul() {
				let a = Quant::from_frac(2, 3);
				let b = Quant::from_frac(3, 4);
				assert_eq!(a * b, Quant::from_frac(6, 12));
			}

			#[test]
			fn test_mul_with_integer() {
				let a = Quant::from_frac(3, 5);
				let b = 2;
				assert_eq!(a * b, Quant::from_frac(6, 5));
			}

			#[test]
			fn test_mul_negative_numbers() {
				let a = Quant::from_frac(-2, 3);
				let b = Quant::from_frac(-3, 4);
				assert_eq!(a * b, Quant::from_frac(6, 12));
			}

			#[test]
			fn test_mul_negative_signs() {
				let a = Quant::from_frac(-2, 3);
				let b = Quant::from_frac(3, 4);
				assert_eq!(a * b, Quant::from_frac(-6, 12));

				let c = Quant::from_frac(2, 3);
				let d = Quant::from_frac(-3, 4);
				assert_eq!(c * d, Quant::from_frac(-6, 12));
			}
		}

		mod mul_assign {
			use super::*;

			#[test]
			fn test_mul_assign() {
				let mut a = Quant::from_frac(3, 4);
				let b = Quant::from_frac(2, 3);
				a *= b;
				assert_eq!(a, Quant::from_frac(6, 12));
			}

			#[test]
			fn test_mul_assign_negative_numbers() {
				let mut a = Quant::from_frac(3, 4);
				let b = Quant::from_frac(2, 3);
				a *= b;
				assert_eq!(a, Quant::from_frac(6, 12));
			}

			#[test]
			fn test_mul_assign_mixed_signs() {
				let mut a = Quant::from_frac(-3, 4);
				let b = Quant::from_frac(2, 3);
				a *= b;
				assert_eq!(a, Quant::from_frac(-6, 12));

				let mut c = Quant::from_frac(-3, 4);
				let d = Quant::from_frac(2, 3);
				c *= d;
				assert_eq!(c, Quant::from_frac(-6, 12));
			}
		}

		mod div {
			use super::*;

			#[test]
			fn test_div_large_positive_numbers() {
				let a = Quant::from_frac(98765432109876543210, 1);
				let b = Quant::from_frac(123456789, 1);
				assert_eq!(
					a / b,
					Quant::from_frac(98765432109876543210, 123456789)
				);
			}

			#[test]
			fn test_div_large_negative_numbers() {
				let a = Quant::from_frac(-98765432109876543210, 1);
				let b = Quant::from_frac(-123456789, 1);
				assert_eq!(
					a / b,
					Quant::from_frac(98765432109876543210, 123456789)
				);
			}

			#[test]
			fn test_div_small_positive_numbers() {
				let a = Quant::from_frac(1, 1000000);
				let b = Quant::from_frac(1, 1000);
				assert_eq!(a / b, Quant::from_frac(1, 1000));
			}

			#[test]
			fn test_div_small_negative_numbers() {
				let a = Quant::from_frac(-1, 1000000);
				let b = Quant::from_frac(1, 1000);
				assert_eq!(a / b, Quant::from_frac(-1, 1000));
			}

			#[test]
			fn test_div_large_and_small_numbers() {
				let a = Quant::from_frac(1000000000000000000, 1);
				let b = Quant::from_frac(1, 1000000000);
				assert_eq!(
					a / b,
					Quant::from_frac(1000000000000000000000000000, 1)
				);
			}

			#[test]
			fn test_div_precision_boundary() {
				let a =
					Quant::from_frac(123456789012345678, 1000000000000000000);
				let b = Quant::from_frac(1, 1000000000);
				assert_eq!(
					a / b,
					Quant::from_frac(123456789012345678, 1000000000)
				);
			}

			#[test]
			fn test_div_zero_dividend() {
				let a = Quant::from_frac(0, 1);
				let b = Quant::from_frac(123456789, 1);
				assert_eq!(a / b, Quant::from_frac(0, 1));
			}

			#[test]
			#[should_panic(expected = "Attempt to divide by zero")]
			fn test_div_zero_divisor() {
				let a = Quant::from_frac(123456789, 1);
				let b = Quant::from_frac(0, 1);
				let _ = a / b;
			}

			#[test]
			fn test_div_exact_result() {
				let a = Quant::from_frac(6, 1);
				let b = Quant::from_frac(2, 1);
				assert_eq!(a / b, Quant::from_frac(3, 1));
			}

			#[test]
			fn test_div_inexact_result() {
				let a = Quant::from_frac(7, 1);
				let b = Quant::from_frac(3, 1);
				assert_eq!(a / b, Quant::from_frac(7, 3));
			}

			#[test]
			fn test_div_rounding_result() {
				let a = Quant::from_frac(123456789, 1);
				let b = Quant::from_frac(1000000, 1);
				assert_eq!(a / b, Quant::from_frac(123456789, 1000000));
			}

			#[test]
			fn test_div_negative_mixed_signs() {
				let a = Quant::from_frac(-5, 2);
				let b = Quant::from_frac(3, 4);
				assert_eq!(a / b, Quant::from_frac(-20, 6));
			}

			#[test]
			fn test_div_negative_and_positive() {
				let a = Quant::from_frac(-5, 4);
				let b = Quant::from_frac(-3, 8);
				assert_eq!(a / b, Quant::from_frac(40, 12));
			}

			#[test]
			fn test_div_near_zero_positive() {
				let a = Quant::from_frac(1, 1000000000);
				let b = Quant::from_frac(1, 1000000000000000);
				assert_eq!(a / b, Quant::from_frac(1000000, 1));
			}

			#[test]
			fn test_div_near_zero_negative() {
				let a = Quant::from_frac(-1, 1000000000);
				let b = Quant::from_frac(1, 1000000000000000);
				assert_eq!(a / b, Quant::from_frac(-1000000, 1));
			}
		}

		mod div_assign {
			use super::*;

			#[test]
			fn test_div_assign_large_positive_numbers() {
				let mut a = Quant::from_frac(98765432109876543210, 1);
				let b = Quant::from_frac(123456789, 1);
				a /= b;
				assert_eq!(
					a,
					Quant::from_frac(98765432109876543210, 123456789)
				);
			}

			#[test]
			fn test_div_assign_large_negative_numbers() {
				let mut a = Quant::from_frac(-98765432109876543210, 1);
				let b = Quant::from_frac(-123456789, 1);
				a /= b;
				assert_eq!(
					a,
					Quant::from_frac(98765432109876543210, 123456789)
				);
			}

			#[test]
			fn test_div_assign_small_positive_numbers() {
				let mut a = Quant::from_frac(1, 1000000);
				let b = Quant::from_frac(1, 1000);
				a /= b;
				assert_eq!(a, Quant::from_frac(1, 1000));
			}

			#[test]
			fn test_div_assign_small_negative_numbers() {
				let mut a = Quant::from_frac(-1, 1000000);
				let b = Quant::from_frac(1, 1000);
				a /= b;
				assert_eq!(a, Quant::from_frac(-1, 1000));
			}

			#[test]
			fn test_div_assign_large_and_small_numbers() {
				let mut a = Quant::from_frac(1000000000000000000, 1);
				let b = Quant::from_frac(1, 1000000000);
				a /= b;
				assert_eq!(
					a,
					Quant::from_frac(1000000000000000000000000000, 1)
				);
			}

			#[test]
			fn test_div_assign_precision_boundary() {
				let mut a =
					Quant::from_frac(123456789012345678, 1000000000000000000);
				let b = Quant::from_frac(1, 1000000000);
				a /= b;
				assert_eq!(a, Quant::from_frac(123456789012345678, 1000000000));
			}

			#[test]
			fn test_div_assign_zero_dividend() {
				let mut a = Quant::from_frac(0, 1);
				let b = Quant::from_frac(123456789, 1);
				a /= b;
				assert_eq!(a, Quant::from_frac(0, 1));
			}

			#[test]
			#[should_panic(expected = "Attempt to divide by zero")]
			fn test_div_assign_zero_divisor() {
				let mut a = Quant::from_frac(123456789, 1);
				let b = Quant::from_frac(0, 1);
				a /= b;
			}

			#[test]
			fn test_div_assign_exact_result() {
				let mut a = Quant::from_frac(6, 1);
				let b = Quant::from_frac(2, 1);
				a /= b;
				assert_eq!(a, Quant::from_frac(3, 1));
			}

			#[test]
			fn test_div_assign_inexact_result() {
				let mut a = Quant::from_frac(7, 1);
				let b = Quant::from_frac(3, 1);
				a /= b;
				assert_eq!(a, Quant::from_frac(7, 3));
			}

			#[test]
			fn test_div_assign_rounding_result() {
				let mut a = Quant::from_frac(123456789, 1);
				let b = Quant::from_frac(1000000, 1);
				a /= b;
				assert_eq!(a, Quant::from_frac(123456789, 1000000));
			}

			#[test]
			fn test_div_assign_negative_mixed_signs() {
				let mut a = Quant::from_frac(-5, 2);
				let b = Quant::from_frac(3, 4);
				a /= b;
				assert_eq!(a, Quant::from_frac(-20, 6));
			}

			#[test]
			fn test_div_assign_negative_and_positive() {
				let mut a = Quant::from_frac(-5, 4);
				let b = Quant::from_frac(-3, 8);
				a /= b;
				assert_eq!(a, Quant::from_frac(40, 12));
			}

			#[test]
			fn test_div_assign_near_zero_positive() {
				let mut a = Quant::from_frac(1, 1000000000);
				let b = Quant::from_frac(1, 1000000000000000);
				a /= b;
				assert_eq!(a, Quant::from_frac(1000000, 1));
			}

			#[test]
			fn test_div_assign_near_zero_negative() {
				let mut a = Quant::from_frac(-1, 1000000000);
				let b = Quant::from_frac(1, 1000000000000000);
				a /= b;
				assert_eq!(a, Quant::from_frac(-1000000, 1));
			}
		}

		mod negation {
			use super::*;

			#[test]
			fn test_negation() {
				let a = Quant::from_frac(3, 4);
				assert_eq!(-a, Quant::from_frac(-3, 4));

				let a = Quant::from_frac(-3, 4);
				assert_eq!(-a, Quant::from_frac(3, 4));

				let a = Quant::from_frac(-3, -4);
				assert_eq!(-a, Quant::from_frac(-3, 4));
			}
		}
	}

	mod ordering {
		use super::*;

		#[test]
		fn test_scalar_greater_equal() {
			let a = Quant::from_frac(5, 2);
			let b = Quant::from_frac(10, 4);
			let c = Quant::from_frac(6, 2);
			let d = Quant::from_frac(4, 2);

			assert!(a >= b, "Expected a >= b (both equal to 2.5)");
			assert!(c >= a, "Expected c >= a (3.0 >= 2.5)");
			assert!(!(d >= a), "Expected d < a (2.0 < 2.5)");
		}

		#[test]
		fn test_scalar_less_equal() {
			let a = Quant::from_frac(5, 2);
			let b = Quant::from_frac(10, 4);
			let c = Quant::from_frac(6, 2);
			let d = Quant::from_frac(4, 2);

			assert!(a <= b, "Expected a <= b (both equal to 2.5)");
			assert!(a <= c, "Expected a <= c (2.5 <= 3.0)");
			assert!(!(a <= d), "Expected a > d (2.5 > 2.0)");
		}

		#[test]
		fn test_scalar_equal_i128() {
			let scalar = Quant::from_frac(10, 2);
			let int_value: i128 = 5;

			assert!(scalar == int_value, "Expected scalar == int_value");
		}

		#[test]
		fn test_i128_equal_scalar() {
			let scalar = Quant::from_frac(10, 2);
			let int_value: i128 = 5;

			assert_eq!(int_value, scalar, "Expected int_value == scalar");
		}

		#[test]
		fn test_scalar_partial_ord_i128() {
			let scalar = Quant::from_frac(15, 2);
			let int_value: i128 = 8;

			assert!(scalar < int_value, "Expected scalar < int_value");
			assert!(int_value > scalar, "Expected int_value > scalar");
		}

		#[test]
		fn test_scalar_partial_ord() {
			let a = Quant::from_frac(7, 2);
			let b = Quant::from_frac(9, 2);
			let c = Quant::from_frac(14, 4);

			assert!(a < b, "Expected a < b (3.5 < 4.5)");
			assert!(b > a, "Expected b > a (4.5 > 3.5)");
			assert!(a == c, "Expected a == c (3.5 == 3.5)");
		}

		#[test]
		fn test_scalar_negative_ordering() {
			let a = Quant::from_frac(-5, 2);
			let b = Quant::from_frac(-10, 4);
			let c = Quant::from_frac(-6, 2);
			let d = Quant::from_frac(-4, 2);

			assert!(a >= b, "Expected a >= b (both equal to -2.5)");
			assert!(a > c, "Expected a > c (-2.5 > -3.0)");
			assert!(a <= d, "Expected a <= d (-2.5 <= -2.0)");
		}

		#[test]
		fn test_scalar_abs_ordering() {
			let a = Quant::from_frac(-5, 2);
			let b = Quant::from_frac(5, 2);
			let c = Quant::from_frac(-6, 2);
			let d = Quant::from_frac(6, 2);

			assert_eq!(a.abs(), b.abs(), "Expected |a| == |b|");
			assert_eq!(c.abs(), d.abs(), "Expected |c| == |d|");
			assert!(
				c.abs() < d.abs() + Quant::from_i128(1),
				"Expected |c| < |d| + 1"
			);
		}
	}

	mod rounding {
		use super::*;

		#[test]
		fn test_round_basic() {
			let mut scalar = Quant {
				numerator: 15,
				denominator: 10,
				is_negative: false,
				render_precision: 0,
			};
			scalar.round(0);
			assert_eq!(scalar.numerator, 2);
			assert_eq!(scalar.denominator, 1);
		}

		#[test]
		fn test_round_half_to_even() {
			let mut scalar = Quant {
				numerator: 155,
				denominator: 100,
				is_negative: false,
				render_precision: 0,
			};
			scalar.round(1);
			assert_eq!(scalar.numerator, 8);
			assert_eq!(scalar.denominator, 5);
		}

		#[test]
		fn test_round_half_to_odd() {
			let mut scalar = Quant {
				numerator: 254,
				denominator: 100,
				is_negative: false,
				render_precision: 0,
			};
			scalar.round(1);
			assert_eq!(scalar.numerator, 5);
			assert_eq!(scalar.denominator, 2);
		}

		#[test]
		fn test_round_resolution_0() {
			let mut scalar = Quant {
				numerator: 7,
				denominator: 3,
				is_negative: false,
				render_precision: 0,
			};
			scalar.round(0);
			assert_eq!(scalar.numerator, 2);
			assert_eq!(scalar.denominator, 1);
		}

		#[test]
		fn test_round_negative() {
			let mut scalar = Quant {
				numerator: 7,
				denominator: 3,
				is_negative: true,
				render_precision: 0,
			};
			scalar.round(0);
			assert_eq!(scalar.numerator, 2);
			assert!(scalar.is_negative);
		}

		#[test]
		fn test_reduce_after_round() {
			let mut scalar = Quant {
				numerator: 200,
				denominator: 100,
				is_negative: false,
				render_precision: 0,
			};
			scalar.round(0);
			assert_eq!(scalar.numerator, 2);
			assert_eq!(scalar.denominator, 1);
		}

		#[test]
		fn test_round_to_integer_no_string() {
			let mut scalar = Quant::from_frac(123456, 1000);
			scalar.round(0);
			assert_eq!(
				scalar.numerator, 123,
				"Numerator should be 123 after rounding to 0 decimals"
			);
			assert_eq!(
				scalar.denominator, 1,
				"Denominator should be 1 after rounding to 0 decimals"
			);
			assert!(!scalar.is_negative, "Scalar should not be negative");
		}

		#[test]
		fn test_round_to_two_decimals_no_string() {
			let mut scalar = Quant::from_frac(123456, 1000);
			scalar.round(2);
			assert_eq!(
				scalar.numerator, 6173,
				"Numerator should be 6173 after rounding to 2 decimals"
			);
			assert_eq!(
				scalar.denominator, 50,
				"Denominator should be 50 after rounding to 2 decimals"
			);
			assert!(!scalar.is_negative, "Scalar should not be negative");
		}

		#[test]
		fn test_bankers_rounding_down_no_string() {
			let mut scalar = Quant::from_frac(123445, 1000);
			scalar.round(2);
			assert_eq!(
				scalar.numerator, 3086,
				"Numerator should be 3086 due to Banker's rounding"
			);
			assert_eq!(
				scalar.denominator, 25,
				"Denominator should remain scaled correctly"
			);
			assert!(!scalar.is_negative, "Scalar should not be negative");
		}

		#[test]
		fn test_bankers_rounding_up_no_string() {
			let mut scalar = Quant::from_frac(123455, 1000);
			scalar.round(2);
			assert_eq!(
				scalar.numerator, 6173,
				"Numerator should be 6173 due to Banker's rounding"
			);
			assert_eq!(
				scalar.denominator, 50,
				"Denominator should remain scaled correctly"
			);
			assert!(!scalar.is_negative, "Scalar should not be negative");
		}

		#[test]
		fn test_round_negative_to_integer_no_string() {
			let mut scalar = Quant::from_frac(-123456, 1000);
			scalar.round(0);
			assert_eq!(
				scalar.numerator, 123,
				"Numerator should be 123 after rounding"
			);
			assert_eq!(
				scalar.denominator, 1,
				"Denominator should be 1 after rounding"
			);
			assert!(scalar.is_negative, "Scalar should be negative");
		}

		#[test]
		fn test_round_negative_to_one_decimal_no_string() {
			let mut scalar = Quant::from_frac(-123456, 1000);
			scalar.round(1);
			assert_eq!(
				scalar.numerator, 247,
				"Numerator should be 247 after rounding"
			);
			assert_eq!(
				scalar.denominator, 2,
				"Denominator should be 2 after rounding to 1 decimal"
			);
			assert!(scalar.is_negative, "Scalar should be negative");
		}

		#[test]
		fn test_round_large_number_no_string() {
			let mut scalar = Quant::from_frac(123456789987654321, 1000000000);
			scalar.round(6);
			assert_eq!(
				scalar.numerator, 61728394993827,
				"Numerator should match rounded value"
			);
			assert_eq!(
				scalar.denominator, 500000,
				"Denominator should match scaled precision"
			);
			assert!(!scalar.is_negative, "Scalar should not be negative");
		}

		#[test]
		fn test_round_small_number_up_no_string() {
			let mut scalar = Quant::from_frac(-5, 10000);
			scalar.round(3);
			assert_eq!(
				scalar.numerator, 0,
				"Numerator should be 0 after rounding down"
			);
			assert_eq!(
				scalar.denominator, 1,
				"Denominator should be set to one when numerator is zero"
			);
			assert!(!scalar.is_negative, "Scalar should not be negative");
		}

		#[test]
		fn test_round_small_number_down_no_string() {
			let mut scalar = Quant::from_frac(49, 100000);
			scalar.round(3);
			assert_eq!(
				scalar.numerator, 0,
				"Numerator should be 0 after rounding down"
			);
			assert_eq!(
				scalar.denominator, 1,
				"Denominator should simplify to 1 for zero value"
			);
			assert!(!scalar.is_negative, "Scalar should not be negative");
		}

		#[test]
		fn test_round_to_integer() {
			let mut scalar = Quant::from_str("123.456").unwrap();
			scalar.round(0);
			assert_eq!(
				scalar.to_string(),
				"123",
				"Expected 123 after rounding to 0 decimals"
			);
		}

		#[test]
		fn test_round_to_two_decimals() {
			let mut scalar = Quant::from_str("123.456").unwrap();
			scalar.round(2);
			assert_eq!(
				scalar.to_string(),
				"123.46",
				"Expected 123.46 after rounding to 2 decimals"
			);
		}

		#[test]
		fn test_bankers_rounding_down() {
			let mut scalar = Quant::from_str("123.445").unwrap();
			scalar.round(2);
			assert_eq!(
				scalar.to_string(),
				"123.44",
				"Expected 123.44 due to Banker's rounding (tie to even)"
			);
		}

		#[test]
		fn test_bankers_rounding_up() {
			let mut scalar = Quant::from_str("123.455").unwrap();
			scalar.round(2);
			assert_eq!(
				scalar.to_string(),
				"123.46",
				"Expected 123.46 due to Banker's rounding (tie to even)"
			);
		}

		#[test]
		fn test_round_negative_to_integer() {
			let mut scalar = Quant::from_str("-123.456").unwrap();
			scalar.round(0);
			assert_eq!(
				scalar.to_string(),
				"-123",
				"Expected -123 after rounding to 0 decimals"
			);
		}

		#[test]
		fn test_round_negative_to_one_decimal() {
			let mut scalar = Quant::from_str("-123.456").unwrap();
			scalar.round(1);
			assert_eq!(
				scalar.to_string(),
				"-123.5",
				"Expected -123.5 after rounding to 1 decimal"
			);
		}

		#[test]
		fn test_round_large_number() {
			let mut scalar = Quant::from_str("123456789.987654321").unwrap();
			scalar.round(6);
			assert_eq!(
				scalar.to_string(),
				"123,456,789.987654",
				"Expected 123,456,789.987654 after rounding to 6 decimals"
			);
		}

		#[test]
		fn test_round_zero() {
			let mut scalar = Quant::from_str("0.0005").unwrap();
			scalar.round(3);
			assert_eq!(
				scalar.to_string(),
				"0.000",
				"Expected bankers rounding to bring us back to zero"
			);
		}

		#[test]
		fn test_round_small_number_down() {
			let mut scalar = Quant::from_str("0.00049").unwrap();
			scalar.round(3);
			assert_eq!(
				scalar.to_string(),
				"0.000",
				"Expected 0.000 after rounding down small value"
			);
		}
	}

	mod other {
		use super::*;

		#[test]
		fn test_display() {
			let money = Quant::from_str("12345.6789").unwrap();
			assert_eq!(money.to_string(), "12,345.6789");

			let negative_money = Quant::from_str("-1000000.50").unwrap();
			assert_eq!(negative_money.to_string(), "-1,000,000.50");

			let zero_money = Quant::from_str("0.00").unwrap();
			assert_eq!(zero_money.to_string(), "0.00")
		}
	}
}
