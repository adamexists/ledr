/* Copyright (C) 2024 Adam House <adam@adamexists.com>
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

use crate::util::amount::Amount;
use crate::util::scalar::Scalar;
use anyhow::{bail, Error};
use std::collections::{HashMap, HashSet};

#[derive(Debug)]
pub struct Graph {
	nodes: HashMap<String, Node>, // currency symbol -> its Node
}

#[derive(Debug)]
struct Node {
	edges: HashMap<String, Rate>, // currency symbol -> conversion rate
}

#[derive(Debug)]
struct Rate {
	pub ratio: Scalar,
	pub rate_type: RateType,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum RateType {
	/// i.e. The user said this is true
	Declared,
	/// i.e. We inferred this rate from an entry or detail
	Inferred,
}

impl Graph {
	pub fn new() -> Self {
		Graph {
			nodes: HashMap::new(),
		}
	}

	/// Adds a bidirectional exchange rate between two currencies
	pub fn add_rate(
		&mut self,
		a: &Amount,
		b: &Amount,
		is_inferred: bool,
	) -> Result<(), Error> {
		let rate_ab = a.value / b.value;
		let rate_ba = b.value / a.value;

		let rate_type = if is_inferred {
			RateType::Inferred
		} else {
			RateType::Declared
		};

		if rate_ab == Scalar::zero() || rate_ba == Scalar::zero() {
			bail!("Exchange rates cannot be zero");
		}

		// Update the graph
		self.nodes
			.entry(a.currency.clone())
			.or_insert_with(Node::new)
			.edges
			.insert(
				b.currency.clone(),
				Rate {
					ratio: rate_ab,
					rate_type,
				},
			);

		self.nodes
			.entry(b.currency.clone())
			.or_insert_with(Node::new)
			.edges
			.insert(
				a.currency.clone(),
				Rate {
					ratio: rate_ba,
					rate_type,
				},
			);

		Ok(())
	}

	pub fn has_inconsistent_cycle(&self) -> bool {
		for currency in self.nodes.keys() {
			let mut visited = HashSet::new();
			let mut rec_stack = HashSet::new();
			if self.detect_inconsistent_cycle(
				currency,
				&mut visited,
				&mut rec_stack,
				1.0,
				currency,
			) {
				return true;
			}
		}

		false
	}

	fn detect_inconsistent_cycle(
		&self,
		current: &str,
		visited: &mut HashSet<String>,
		rec_stack: &mut HashSet<String>,
		rate_product: f64,
		start: &str, // Track the original starting node
	) -> bool {
		if rec_stack.contains(current) {
			// Cycle detected, but only check consistency if we're back at the starting node
			if current == start {
				return !(0.95..=1.05).contains(&rate_product);
			} else {
				return false;
			}
		}

		if visited.contains(current) {
			return false;
		}

		// Mark the current node as visited and add to the recursion stack
		visited.insert(current.to_string());
		rec_stack.insert(current.to_string());

		if let Some(node) = self.nodes.get(current) {
			for (neighbor, rate) in &node.edges {
				let new_rate_product =
					rate_product * rate.ratio.as_f64();

				// Recursive call for the neighbor
				if self.detect_inconsistent_cycle(
					neighbor,
					visited,
					rec_stack,
					new_rate_product,
					start, // Always pass the original starting node
				) {
					return true;
				}
			}
		}

		// Backtrack: Remove from recursion stack
		rec_stack.remove(current);

		false
	}

	/// Reports the rate between two currencies, with base-quote semantics. None if no path
	/// exists in the graph between the currencies, else there will always be a result.
	///
	/// If there is a direct rate between currencies, we use Scalar math. If at least one
	/// indirect hop is required, we fall back to f64 math at a precision higher than the
	/// maximum supported by Scalar. This TODO should be documented extensively.
	pub fn convert(&self, base: &str, quote: &str) -> Option<Scalar> {
		if let Some(direct) = self.get_direct_rate(base, quote, false) {
			return Some(direct);
		}

		let mut visited = HashMap::new();
		let mut queue = vec![(base.to_string(), 1f64)];

		while let Some((current_currency, current_rate)) = queue.pop() {
			if let Some(node) = self.nodes.get(&current_currency) {
				for (neighbor, rate) in &node.edges {
					if !visited.contains_key(neighbor) {
						let new_rate = current_rate
							* rate.ratio.as_f64();

						if neighbor == quote {
							return Some(Scalar::from_f64(new_rate));
						}

						visited.insert(
							neighbor.clone(),
							new_rate,
						);
						queue.push((
							neighbor.clone(),
							new_rate,
						));
					}
				}
			}
		}

		None
	}

	/// Reports whether two currency nodes are adjacent. If they are, it will still report
	/// false if must_be_declared is true and the given rate is not declared.
	pub fn get_direct_rate(
		&self,
		base: &str,
		quote: &str,
		must_be_declared: bool,
	) -> Option<Scalar> {
		if base == quote {
			return Some(Scalar::from_i128(1));
		}

		let node = match self.nodes.get(quote) {
			Some(node) => node,
			None => return None,
		};

		let other = match node.edges.get(base) {
			Some(other) => other,
			None => return None,
		};

		if must_be_declared && other.rate_type != RateType::Declared {
			return None;
		};

		Some(other.ratio)
	}

	/// Returns all rates as tuples of (base, quote, rate)
	/// This includes both direct and indirect conversion rates.
	pub fn get_all_rates(&self) -> Vec<(String, String, Scalar)> {
		let mut rates = Vec::new();

		for base in self.nodes.keys() {
			for quote in self.nodes.keys() {
				if base != quote {
					if let Some(rate) =
						self.convert(base, quote)
					{
						rates.push((
							base.clone(),
							quote.clone(),
							rate,
						));
					}
				}
			}
		}

		rates
	}
}

impl Node {
	fn new() -> Self {
		Node {
			edges: HashMap::new(),
		}
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use crate::util::scalar::Scalar;

	#[test]
	fn test_direct_conversion() {
		let mut graph = Graph::new();
		let a = Amount::new(Scalar::new(2, 0), "USD".to_string());
		let b = Amount::new(Scalar::new(1, 0), "EUR".to_string());
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let expected_output = Scalar::new(5, 1);
		let result = graph.convert("USD", "EUR");

		assert!(result.is_some());
		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_reverse_conversion() {
		let mut graph = Graph::new();
		let a = Amount::new(Scalar::new(2, 0), "USD".to_string());
		let b = Amount::new(Scalar::new(1, 0), "EUR".to_string());
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let expected_output = Scalar::new(2, 0);
		let result = graph.convert("EUR", "USD");

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_indirect_conversion() {
		let mut graph = Graph::new();
		let a = Amount::new(Scalar::new(2, 0), "USD".to_string());
		let b = Amount::new(Scalar::new(1, 0), "EUR".to_string());
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let c = Amount::new(Scalar::new(5, 0), "EUR".to_string());
		let d = Amount::new(Scalar::new(1, 0), "GBP".to_string());
		graph.add_rate(&c, &d, true).expect("Could not add rate");

		let expected_output = Scalar::new(10, 0);
		let result = graph.convert("USD", "GBP");

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_reverse_indirect_conversion() {
		let mut graph = Graph::new();
		let a = Amount::new(Scalar::new(2, 0), "USD".to_string());
		let b = Amount::new(Scalar::new(1, 0), "EUR".to_string());
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let c = Amount::new(Scalar::new(5, 0), "EUR".to_string());
		let d = Amount::new(Scalar::new(1, 0), "GBP".to_string());
		graph.add_rate(&c, &d, true).expect("Could not add rate");

		let expected_output = Scalar::new(1, 1);
		let result = graph.convert("GBP", "USD");

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_self_conversion() {
		let graph = Graph::new();

		let result = graph.convert("USD", "USD");

		assert_eq!(result.unwrap(), Scalar::from_i128(1));
	}

	#[test]
	fn test_no_conversion_path() {
		let mut graph = Graph::new();
		let a = Amount::new(Scalar::new(2, 0), "USD".to_string());
		let b = Amount::new(Scalar::new(1, 0), "EUR".to_string());
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let c = Amount::new(Scalar::new(3, 0), "JPY".to_string());
		let d = Amount::new(Scalar::new(1, 0), "INR".to_string());
		graph.add_rate(&c, &d, true).expect("Could not add rate");

		let result = graph.convert("USD", "JPY");

		assert!(result.is_none());
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_large_graph_conversion() {
		let mut graph = Graph::new();
		let a = Amount::new(Scalar::new(2, 0), "USD".to_string());
		let b = Amount::new(Scalar::new(1, 0), "EUR".to_string());
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let c = Amount::new(Scalar::new(5, 0), "EUR".to_string());
		let d = Amount::new(Scalar::new(1, 0), "GBP".to_string());
		graph.add_rate(&c, &d, true).expect("Could not add rate");

		let e = Amount::new(Scalar::new(10, 0), "GBP".to_string());
		let f = Amount::new(Scalar::new(1, 0), "JPY".to_string());
		graph.add_rate(&e, &f, true).expect("Could not add rate");

		let g = Amount::new(Scalar::new(3, 0), "JPY".to_string());
		let h = Amount::new(Scalar::new(1, 0), "INR".to_string());
		graph.add_rate(&g, &h, true).expect("Could not add rate");

		let expected_output = Scalar::new(300, 0);
		let result = graph.convert("USD", "INR");
		let direct_rate =
			graph.get_direct_rate("GBP", "EUR", false).unwrap();

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
		assert_eq!(direct_rate, Scalar::new(5, 0));
	}

	#[test]
	fn test_nonexistent_currency() {
		let graph = Graph::new();

		let result = graph.convert("USD", "XYZ");

		assert!(result.is_none());
	}

	#[test]
	fn test_inconsistent_cycle_detection() {
		let mut graph = Graph::new();

		// Add USD <-> EUR rate
		let a = Amount::new(Scalar::new(2, 0), "USD".to_string());
		let b = Amount::new(Scalar::new(1, 0), "EUR".to_string());
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		// Add EUR <-> GBP rate
		let c = Amount::new(Scalar::new(5, 0), "EUR".to_string());
		let d = Amount::new(Scalar::new(1, 0), "GBP".to_string());
		graph.add_rate(&c, &d, true).expect("Could not add rate");

		// Add GBP <-> USD rate that creates an inconsistent cycle
		let e = Amount::new(Scalar::new(1, 0), "GBP".to_string());
		let f = Amount::new(Scalar::new(1, 0), "USD".to_string());
		let _result = graph.add_rate(&e, &f, true);

		// Assert that the inconsistent cycle is detected
		assert!(graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_large_connected_graph_no_cycles() {
		let mut graph = Graph::new();

		// Generate many currencies
		let currencies: Vec<String> =
			(1..=200).map(|i| format!("C{i:03}")).collect();

		// Add consistent bidirectional rates
		for i in 0..currencies.len() {
			for j in i + 1..currencies.len() {
				let rate1 = Scalar::from_i128((i + 1) as i128);
				let rate2 = Scalar::from_i128((j + 1) as i128);
				let a = Amount::new(
					rate1,
					currencies[i].to_string(),
				);
				let b = Amount::new(
					rate2,
					currencies[j].to_string(),
				);
				graph.add_rate(&a, &b, true)
					.expect("Could not add rate");
			}
		}

		// Test for inconsistent cycles
		assert!(
			!graph.has_inconsistent_cycle(),
			"No inconsistent cycles expected"
		);
	}

	#[test]
	fn test_large_connected_graph_with_contextual_inconsistencies() {
		let mut graph = Graph::new();

		// Add bidirectional rates between some currencies
		let pairs = vec![
			("USD", "EUR", Scalar::new(120, 2)), // 1 USD = 1.2 EUR
			("EUR", "GBP", Scalar::new(90, 2)),  // 1 EUR = 0.9 GBP
			("GBP", "USD", Scalar::new(110, 2)), // 1 GBP = 1.1 USD (creates inconsistency)
			("JPY", "USD", Scalar::new(8, 1)),   // 1 JPY = 0.8 USD
			("AUD", "JPY", Scalar::new(70, 2)),  // 1 AUD = 0.7 JPY
			("CAD", "AUD", Scalar::new(100, 2)), // 1 CAD = 1.0 AUD
			("CHF", "CAD", Scalar::new(95, 2)),  // 1 CHF = 0.95 CAD
			("CNY", "CHF", Scalar::new(10, 1)),  // 1 CNY = 0.1 CHF
			("INR", "CNY", Scalar::new(6, 1)),   // 1 INR = 0.6 CNY
			("SGD", "INR", Scalar::new(80, 2)),  // 1 SGD = 0.8 INR
		];

		// Add all bidirectional rates
		for (from, to, rate) in pairs {
			let amount_from = Amount::new(
				Scalar::new(100, 0),
				from.to_string(),
			);
			let amount_to = Amount::new(rate, to.to_string());
			graph.add_rate(&amount_from, &amount_to, true)
				.expect("Could not add rate");
		}

		// Test for inconsistent cycles
		assert!(
			graph.has_inconsistent_cycle(),
			"Expected inconsistent cycle in graph"
		);
	}

	#[test]
	fn test_disconnected_segments_with_contextual_inconsistencies() {
		let mut graph = Graph::new();

		// Define three disconnected segments
		let segment1 = vec![
			("USD", "EUR", Scalar::new(120, 2)),
			("EUR", "GBP", Scalar::new(90, 2)),
			("GBP", "USD", Scalar::new(110, 2)), // Inconsistent cycle
		];
		let segment2 = vec![
			("JPY", "AUD", Scalar::new(70, 2)),
			("AUD", "CAD", Scalar::new(100, 2)),
			("CAD", "JPY", Scalar::new(150, 2)), // Inconsistent cycle
		];
		let segment3 = vec![
			("INR", "CNY", Scalar::new(6, 1)),
			("CNY", "CHF", Scalar::new(10, 1)),
			("CHF", "INR", Scalar::new(50, 1)), // Inconsistent cycle
		];

		// Add rates for each segment
		for (from, to, rate) in segment1 {
			let amount_from = Amount::new(
				Scalar::new(100, 0),
				from.to_string(),
			);
			let amount_to = Amount::new(rate, to.to_string());
			graph.add_rate(&amount_from, &amount_to, true)
				.expect("Could not add rate");
		}
		for (from, to, rate) in segment2 {
			let amount_from = Amount::new(
				Scalar::new(100, 0),
				from.to_string(),
			);
			let amount_to = Amount::new(rate, to.to_string());
			graph.add_rate(&amount_from, &amount_to, true)
				.expect("Could not add rate");
		}
		for (from, to, rate) in segment3 {
			let amount_from = Amount::new(
				Scalar::new(100, 0),
				from.to_string(),
			);
			let amount_to = Amount::new(rate, to.to_string());
			graph.add_rate(&amount_from, &amount_to, true)
				.expect("Could not add rate");
		}

		// Test for inconsistent cycles
		assert!(
			graph.has_inconsistent_cycle(),
			"Expected inconsistent cycles in segments"
		);
	}

	#[test]
	fn test_multiple_disconnected_segments_no_cycles() {
		let mut graph = Graph::new();

		// Segment 1
		let seg1 = ["USD", "EUR", "GBP"];
		for i in 0..seg1.len() {
			for j in i + 1..seg1.len() {
				let a = Amount::new(
					Scalar::from_i128((i + 1) as i128),
					seg1[i].to_string(),
				);
				let b = Amount::new(
					Scalar::from_i128((j + 1) as i128),
					seg1[j].to_string(),
				);
				graph.add_rate(&a, &b, true)
					.expect("Could not add rate");
			}
		}

		// Segment 2
		let seg2 = ["JPY", "AUD", "CAD"];
		for i in 0..seg2.len() {
			for j in i + 1..seg2.len() {
				let a = Amount::new(
					Scalar::from_i128((i + 1) as i128),
					seg2[i].to_string(),
				);
				let b = Amount::new(
					Scalar::from_i128((j + 1) as i128),
					seg2[j].to_string(),
				);
				graph.add_rate(&a, &b, true)
					.expect("Could not add rate");
			}
		}

		// Segment 3
		let seg3 = ["CHF", "CNY", "INR"];
		for i in 0..seg3.len() {
			for j in i + 1..seg3.len() {
				let a = Amount::new(
					Scalar::from_i128((i + 1) as i128),
					seg3[i].to_string(),
				);
				let b = Amount::new(
					Scalar::from_i128((j + 1) as i128),
					seg3[j].to_string(),
				);
				graph.add_rate(&a, &b, true)
					.expect("Could not add rate");
			}
		}

		// Test for inconsistent cycles
		assert!(
			!graph.has_inconsistent_cycle(),
			"No inconsistent cycles expected"
		);
	}
}
