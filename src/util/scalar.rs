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
use std::hash::Hash;
use std::iter::Sum;
use std::ops::{
	Add, AddAssign, Div, DivAssign, Mul, MulAssign, Neg, Sub, SubAssign,
};

/// A general-purpose number, capable of holding an exact decimal value, backed
/// by integer arithmetic and not float arithmetic for addition and subtraction.
#[derive(Clone, Copy, Debug, Default, Hash)]
pub struct Scalar {
	numerator: i128,
	denominator: i128,

	/// How many decimal places to render when asked to print. Will round with
	/// banker's rounding when underlying precision exceeds what is requested.
	/// TODO: Implement / verify such rounding.
	render_precision: u32,
}

impl Scalar {
	pub fn zero() -> Self {
		Self {
			numerator: 0,
			denominator: 1,
			render_precision: 0,
		}
	}

	// TODO: Rework this new method.
	pub fn new(number: i128, render_precision: u32) -> Self {
		let mut out = Self {
			numerator: number,
			denominator: 1 * 10i128.pow(render_precision),
			render_precision,
		};
		out.reduce();
		out
	}

	fn from_frac_with_render_precision(
		numerator: i128,
		denominator: i128,
		render_precision: u32,
	) -> Self {
		let mut out = Self {
			numerator,
			denominator,
			render_precision,
		};
		out.reduce();
		out
	}

	fn from_frac(numerator: i128, denominator: i128) -> Self {
		if denominator == 0 {
			panic!("Denominator cannot be zero");
		}
		let gcd = Self::gcd(numerator, denominator);
		let sign = if denominator < 0 { -1 } else { 1 };

		let denominator = denominator.abs() / gcd;
		Self {
			numerator: numerator / gcd * sign,
			denominator,
			render_precision: Self::num_digits(denominator.try_into().unwrap())
				as u32,
		}
	}

	pub fn resolution(&self) -> u32 {
		self.render_precision
	}

	// TODO: Switch this out in various places.
	fn from_decimal(whole_part: i128, decimal_part: i128) -> Self {
		let precision = Self::num_digits(decimal_part) as u32;
		let whole = whole_part;
		let scale = 10i128.pow(precision);
		Self {
			numerator: whole * scale + decimal_part,
			denominator: scale,
			render_precision: precision,
		}
	}

	fn num_digits(mut n: i128) -> i128 {
		let mut digits = 0;
		loop {
			digits += 1;
			if n < 10 {
				break;
			}
			n /= 10;
		}
		digits
	}

	pub fn from_i128(amount: i128) -> Self {
		Self {
			numerator: amount,
			denominator: 1,
			render_precision: 0,
		}
	}

	pub fn from_str(amount: &str) -> Result<Self, Error> {
		// Remove commas from the string
		let sanitized: String = amount.chars().filter(|c| *c != ',').collect();

		// Check for negative sign explicitly and removing it for parsing
		let is_negative = sanitized.starts_with('-');
		let sanitized = sanitized.trim_start_matches('-');

		let parts: Vec<&str> = sanitized.split('.').collect();
		let mut precision = 0u32;

		let (numerator, denominator) = match parts.len() {
			1 => {
				let value = parts[0].parse::<i128>()?;
				(if is_negative { -value } else { value }, 1)
			},
			2 => {
				let whole = parts[0].parse::<i128>()?;
				let decimal = parts[1];
				precision = decimal.len() as u32;
				let scale = 10i128.pow(precision);
				let fractional = decimal.parse::<i128>()?;
				let numerator = whole * scale + fractional;
				(if is_negative { -numerator } else { numerator }, scale)
			},
			_ => bail!("Invalid decimal format"),
		};

		Ok(Self::from_frac_with_render_precision(
			numerator,
			denominator,
			precision,
		))
	}

	// TODO: I do not want this to exist for long.
	pub fn from_f64(value: f64) -> Self {
		if value.is_nan() {
			panic!("Cannot create a Scalar from NaN");
		}

		if value.is_infinite() {
			panic!("Cannot create a Scalar from infinity");
		}

		let is_negative = value < 0.0;
		let abs_value = value.abs();

		// Split the number into integer and fractional parts
		let whole_part = abs_value.floor() as i128;
		let fractional_part = abs_value - whole_part as f64;

		// Approximate the fractional part as a rational number
		let mut numerator = (fractional_part * 10f64.powi(18)) as i128;
		let denominator = 10i128.pow(18);

		// Simplify the fraction
		let gcd = Self::gcd(numerator, denominator);
		numerator /= gcd;
		let denominator = denominator / gcd;

		// Combine whole and fractional parts
		let final_numerator = whole_part * denominator + numerator;
		let final_numerator = if is_negative {
			-final_numerator
		} else {
			final_numerator
		};

		Self::from_frac(final_numerator, denominator)
	}

	/// Tells the Scalar to render with this many decimal places, rounding if
	/// necessary using Banker's rounding (round to nearest, ties to even).
	pub fn round(&mut self, resolution: u32) {
		if resolution == 0 {
			// Handle special case for integer rounding
			let mut quotient = self.numerator / self.denominator;
			let remainder = self.numerator % self.denominator;
			let half_denom = self.denominator / 2;

			if remainder.abs() > half_denom
				|| (remainder.abs() == half_denom && quotient % 2 != 0)
			{
				quotient += self.numerator.signum();
			}

			self.numerator = quotient;
			self.denominator = 1;
			self.render_precision = 0;
			return;
		}

		// Scale the denominator to match the desired precision.
		let scale = 10i128.pow(resolution);
		let scaled_numerator = self.numerator * scale;
		let quotient = scaled_numerator / self.denominator;
		let remainder = scaled_numerator % self.denominator;

		// Perform Banker's rounding.
		let half_denom = self.denominator / 2;
		let rounded_quotient = if remainder.abs() > half_denom
			|| (remainder.abs() == half_denom && quotient % 2 != 0)
		{
			quotient + self.numerator.signum()
		} else {
			quotient
		};

		// Update the scalar to reflect the rounded value.
		self.numerator = rounded_quotient;
		self.denominator = scale;
		self.render_precision = resolution;

		// Ensure the fraction is reduced.
		self.reduce();
	}

	pub fn abs(&self) -> Self {
		Self {
			numerator: self.numerator.abs(),
			denominator: self.denominator,
			render_precision: self.render_precision,
		}
	}

	pub fn negate(&mut self) {
		self.numerator = -self.numerator;
	}

	fn reduce(&mut self) {
		let gcd = Self::gcd(self.numerator, self.denominator);
		self.numerator /= gcd;
		self.denominator /= gcd;

		// Ensure denominator is always positive for normalized form
		if self.denominator < 0 {
			self.numerator = -self.numerator;
			self.denominator = -self.denominator;
		}
	}

	pub fn as_f64(&self) -> f64 {
		self.numerator as f64 / self.denominator as f64
	}

	fn gcd(mut a: i128, mut b: i128) -> i128 {
		while b != 0 {
			let temp = b;
			b = a % b;
			a = temp;
		}
		a.abs()
	}
}

// TODO: Make this implementation a little cleaner.
impl fmt::Display for Scalar {
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		let mut numerator = self.numerator.abs();
		let denominator = self.denominator;

		// Integer part of the number
		let integer_part = numerator / denominator;
		numerator %= denominator;

		// Compute fractional part with precision
		let mut fraction_str = String::new();
		let mut remainder = numerator;
		let precision = f.precision().unwrap_or(self.render_precision as usize);
		for _ in 0..precision {
			remainder *= 10;
			let digit = remainder / denominator;
			remainder %= denominator;
			fraction_str.push(std::char::from_digit(digit as u32, 10).unwrap());
			if remainder == 0 {
				break; // Stop if remainder is zero
			}
		}

		if fraction_str.len() < self.render_precision as usize {
			let zeros_to_add =
				self.render_precision as usize - fraction_str.len();
			fraction_str.push_str(&"0".repeat(zeros_to_add));
		}

		// Remove trailing zeros in the fraction
		while fraction_str.ends_with('0')
			&& fraction_str.len() > self.render_precision as usize
		{
			fraction_str.pop();
		}

		// Format integer part with commas
		let mut int_str = integer_part.to_string();
		let mut i = int_str.len() as isize - 3;
		while i > 0 {
			int_str.insert(i as usize, ',');
			i -= 3;
		}

		// Combine parts
		let formatted = if fraction_str.is_empty() {
			int_str
		} else {
			format!("{}.{}", int_str, fraction_str)
		};

		// Add sign if the original number is negative
		if self.numerator < 0 {
			write!(f, "-{}", formatted)
		} else {
			write!(f, "{}", formatted)
		}
	}
}

// -----------------
// -- BOILERPLATE --
// -----------------

impl Add for Scalar {
	type Output = Self;

	fn add(self, rhs: Self) -> Self::Output {
		let numerator =
			self.numerator * rhs.denominator + rhs.numerator * self.denominator;
		let denominator = self.denominator * rhs.denominator;

		let render_precision = self.render_precision.max(rhs.render_precision);

		let mut out = Self::from_frac_with_render_precision(
			numerator,
			denominator,
			render_precision,
		);
		out.reduce();
		out
	}
}

impl AddAssign for Scalar {
	fn add_assign(&mut self, rhs: Self) {
		*self = *self + rhs; // TODO: Redo this for efficiency.
	}
}

impl Sum for Scalar {
	fn sum<I: Iterator<Item = Self>>(iter: I) -> Self {
		iter.fold(Scalar::zero(), |acc, scalar| acc + scalar)
	}
}

impl Sub for Scalar {
	type Output = Self;

	fn sub(self, rhs: Self) -> Self::Output {
		let numerator =
			self.numerator * rhs.denominator - rhs.numerator * self.denominator;
		let denominator = self.denominator * rhs.denominator;

		let render_precision = self.render_precision.max(rhs.render_precision);

		let mut out = Self::from_frac_with_render_precision(
			numerator,
			denominator,
			render_precision,
		);
		out.reduce();
		out
	}
}

impl SubAssign for Scalar {
	fn sub_assign(&mut self, rhs: Self) {
		*self = *self - rhs; // TODO: Redo this for efficiency.
	}
}

impl Mul for Scalar {
	type Output = Self;

	fn mul(self, rhs: Self) -> Self::Output {
		let numerator = self.numerator * rhs.numerator;
		let denominator = self.denominator * rhs.denominator;

		let render_precision = self.render_precision.max(rhs.render_precision);

		let mut out = Self::from_frac_with_render_precision(
			numerator,
			denominator,
			render_precision,
		);
		out.reduce();
		out
	}
}

impl MulAssign for Scalar {
	fn mul_assign(&mut self, rhs: Self) {
		*self = *self * rhs; // TODO: Redo this for efficiency.
	}
}

impl Div for Scalar {
	type Output = Self;

	fn div(self, rhs: Self) -> Self::Output {
		if rhs.numerator == 0 {
			panic!("Attempt to divide by zero");
		}
		let numerator = self.numerator * rhs.denominator;
		let denominator = self.denominator * rhs.numerator;

		let render_precision = self.render_precision.max(rhs.render_precision);

		let mut out = Self::from_frac_with_render_precision(
			numerator,
			denominator,
			render_precision,
		);
		out.reduce();
		out
	}
}

impl DivAssign for Scalar {
	fn div_assign(&mut self, rhs: Self) {
		*self = *self / rhs; // TODO: Redo this for efficiency.
	}
}

impl Neg for Scalar {
	type Output = Self;

	fn neg(self) -> Self::Output {
		Self {
			numerator: -self.numerator,
			denominator: self.denominator,
			render_precision: self.render_precision,
		}
	}
}

impl PartialEq<i128> for Scalar {
	fn eq(&self, &other: &i128) -> bool {
		self.numerator == other * self.denominator
	}
}

impl PartialEq for Scalar {
	fn eq(&self, other: &Self) -> bool {
		self.numerator * other.denominator == other.numerator * self.denominator
	}
}

impl PartialEq<Scalar> for i128 {
	fn eq(&self, other: &Scalar) -> bool {
		*self * other.denominator == other.numerator
	}
}

impl Eq for Scalar {}

impl PartialOrd for Scalar {
	fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
		Some(self.cmp(other))
	}
}

impl PartialOrd<i128> for Scalar {
	fn partial_cmp(&self, &other: &i128) -> Option<Ordering> {
		Some((self.numerator).cmp(&(other * self.denominator)))
	}
}

impl PartialOrd<Scalar> for i128 {
	fn partial_cmp(&self, other: &Scalar) -> Option<Ordering> {
		Some((*self * other.denominator).cmp(&other.numerator))
	}
}

impl Ord for Scalar {
	fn cmp(&self, other: &Self) -> Ordering {
		(self.numerator * other.denominator)
			.cmp(&(other.numerator * self.denominator))
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	mod basics {
		use super::*;

		#[test]
		fn test_scalar_creation() {
			let scalar = Scalar::from_frac(3, 4);
			assert_eq!(scalar.numerator, 3);
			assert_eq!(scalar.denominator, 4);
		}

		#[test]
		#[should_panic(expected = "Denominator cannot be zero")]
		fn test_scalar_creation_zero_denominator() {
			Scalar::from_frac(1, 0);
		}

		#[test]
		fn test_scalar_reduction() {
			let scalar = Scalar::from_frac(6, 8);
			assert_eq!(scalar.numerator, 3);
			assert_eq!(scalar.denominator, 4);
		}
	}

	mod other {
		use super::*;

		#[test]
		fn test_display() {
			let money = Scalar::from_str("12345.6789").unwrap();
			assert_eq!(money.to_string(), "12,345.6789");

			let negative_money = Scalar::from_str("-1000000.50").unwrap();
			assert_eq!(negative_money.to_string(), "-1,000,000.50");

			let zero_money = Scalar::from_str("0.00").unwrap();
			assert_eq!(zero_money.to_string(), "0.00")
		}
	}
}
