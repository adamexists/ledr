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

use crate::util::amount::Amount;
use crate::util::quant::Quant;
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
	pub ratio: Quant,
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

		if rate_ab == Quant::zero() || rate_ba == Quant::zero() {
			bail!("Exchange rates cannot be zero");
		}

		let rate_type = if is_inferred {
			RateType::Inferred
		} else {
			RateType::Declared
		};

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
				Quant::from_i128(1),
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
		rate_product: Quant,
		start: &str, // Track the original starting node
	) -> bool {
		if rec_stack.contains(current) {
			// Cycle detected, but only check consistency if we're back at the starting node
			if current == start {
				return rate_product > Quant::from_frac(21, 20)
					|| rate_product < Quant::from_frac(19, 20);
			} else {
				false
			};
		}

		if visited.contains(current) {
			return false;
		}

		// Mark the current node as visited and add to the recursion stack
		visited.insert(current.to_string());
		rec_stack.insert(current.to_string());

		if let Some(node) = self.nodes.get(current) {
			for (neighbor, rate) in &node.edges {
				let new_rate_product = rate_product * rate.ratio;

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
	pub fn convert(&self, base: &str, quote: &str) -> Option<Quant> {
		if base == quote {
			return Some(Quant::from_frac(1, 1));
		}

		let mut visited = HashMap::new();
		let mut queue = vec![(quote.to_string(), Quant::from_i128(1))];

		while let Some((current_currency, current_rate)) = queue.pop() {
			if let Some(node) = self.nodes.get(&current_currency) {
				for (neighbor, rate) in &node.edges {
					if !visited.contains_key(neighbor) {
						let new_rate = current_rate * rate.ratio;

						if neighbor == base {
							return Some(new_rate);
						}

						visited.insert(neighbor.clone(), new_rate);
						queue.push((neighbor.clone(), new_rate));
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
	) -> Option<Quant> {
		if base == quote {
			return Some(Quant::from_i128(1));
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
	pub fn get_all_rates(&self) -> Vec<(String, String, Quant)> {
		let mut rates = Vec::new();

		for base in self.nodes.keys() {
			for quote in self.nodes.keys() {
				if base != quote {
					if let Some(rate) = self.convert(base, quote) {
						rates.push((base.clone(), quote.clone(), rate));
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
	use crate::util::quant::Quant;

	#[test]
	fn test_direct_conversion() {
		let mut graph = Graph::new();
		let a = Amount::new(Quant::new(2, 0), "USD");
		let b = Amount::new(Quant::new(1, 0), "EUR");
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let expected_output = Quant::new(5, 1);
		let result = graph.convert("USD", "EUR");

		assert!(result.is_some());
		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_reverse_conversion() {
		let mut graph = Graph::new();
		let a = Amount::new(Quant::new(2, 0), "USD");
		let b = Amount::new(Quant::new(1, 0), "EUR");
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let expected_output = Quant::new(2, 0);
		let result = graph.convert("EUR", "USD");

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_indirect_conversion() {
		let mut graph = Graph::new();
		let a = Amount::new(Quant::new(2, 0), "USD");
		let b = Amount::new(Quant::new(1, 0), "EUR");
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let c = Amount::new(Quant::new(5, 0), "EUR");
		let d = Amount::new(Quant::new(1, 0), "GBP");
		graph.add_rate(&c, &d, true).expect("Could not add rate");

		let expected_output = Quant::new(1, 1);
		let result = graph.convert("USD", "GBP");

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_reverse_indirect_conversion() {
		let mut graph = Graph::new();
		let a = Amount::new(Quant::new(2, 0), "USD");
		let b = Amount::new(Quant::new(1, 0), "EUR");
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let c = Amount::new(Quant::new(5, 0), "EUR");
		let d = Amount::new(Quant::new(1, 0), "GBP");
		graph.add_rate(&c, &d, true).expect("Could not add rate");

		let expected_output = Quant::new(10, 0);
		let result = graph.convert("GBP", "USD");

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_self_conversion() {
		let graph = Graph::new();

		let result = graph.convert("USD", "USD");

		assert_eq!(result.unwrap(), Quant::from_i128(1));
	}

	#[test]
	fn test_no_conversion_path() {
		let mut graph = Graph::new();
		let a = Amount::new(Quant::new(2, 0), "USD");
		let b = Amount::new(Quant::new(1, 0), "EUR");
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let c = Amount::new(Quant::new(3, 0), "JPY");
		let d = Amount::new(Quant::new(1, 0), "INR");
		graph.add_rate(&c, &d, true).expect("Could not add rate");

		let result = graph.convert("USD", "JPY");

		assert!(result.is_none());
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_large_graph_conversion() {
		let mut graph = Graph::new();
		let a = Amount::new(Quant::new(2, 0), "USD");
		let b = Amount::new(Quant::new(1, 0), "EUR");
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let c = Amount::new(Quant::new(5, 0), "EUR");
		let d = Amount::new(Quant::new(1, 0), "GBP");
		graph.add_rate(&c, &d, true).expect("Could not add rate");

		let e = Amount::new(Quant::new(10, 0), "GBP");
		let f = Amount::new(Quant::new(1, 0), "JPY");
		graph.add_rate(&e, &f, true).expect("Could not add rate");

		let g = Amount::new(Quant::new(3, 0), "JPY");
		let h = Amount::new(Quant::new(1, 0), "INR");
		graph.add_rate(&g, &h, true).expect("Could not add rate");

		let expected_output = Quant::new(300, 0);
		let result = graph.convert("INR", "USD");
		let direct_rate = graph.get_direct_rate("GBP", "EUR", false).unwrap();

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
		assert_eq!(direct_rate, Quant::new(5, 0));
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
		let a = Amount::new(Quant::new(2, 0), "USD");
		let b = Amount::new(Quant::new(1, 0), "EUR");
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		// Add EUR <-> GBP rate
		let c = Amount::new(Quant::new(5, 0), "EUR");
		let d = Amount::new(Quant::new(1, 0), "GBP");
		graph.add_rate(&c, &d, true).expect("Could not add rate");

		// Add GBP <-> USD rate that creates an inconsistent cycle
		let e = Amount::new(Quant::new(1, 0), "GBP");
		let f = Amount::new(Quant::new(1, 0), "USD");
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
				let rate1 = Quant::from_i128((i * 10 + 3) as i128);
				let rate2 = Quant::from_i128((j * 10 + 3) as i128);
				let a = Amount::new(rate1, &currencies[i]);
				let b = Amount::new(rate2, &currencies[j]);
				graph.add_rate(&a, &b, true).expect("Could not add rate");
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
			("USD", "EUR", Quant::new(120, 2)), // 1 USD = 1.2 EUR
			("EUR", "GBP", Quant::new(90, 2)),  // 1 EUR = 0.9 GBP
			("GBP", "USD", Quant::new(110, 2)), // 1 GBP = 1.1 USD (creates inconsistency)
			("JPY", "USD", Quant::new(8, 1)),   // 1 JPY = 0.8 USD
			("AUD", "JPY", Quant::new(70, 2)),  // 1 AUD = 0.7 JPY
			("CAD", "AUD", Quant::new(100, 2)), // 1 CAD = 1.0 AUD
			("CHF", "CAD", Quant::new(95, 2)),  // 1 CHF = 0.95 CAD
			("CNY", "CHF", Quant::new(10, 1)),  // 1 CNY = 0.1 CHF
			("INR", "CNY", Quant::new(6, 1)),   // 1 INR = 0.6 CNY
			("SGD", "INR", Quant::new(80, 2)),  // 1 SGD = 0.8 INR
		];

		// Add all bidirectional rates
		for (from, to, rate) in pairs {
			let amount_from = Amount::new(Quant::new(100, 0), from);
			let amount_to = Amount::new(rate, to);
			graph
				.add_rate(&amount_from, &amount_to, true)
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
			("USD", "EUR", Quant::new(120, 2)),
			("EUR", "GBP", Quant::new(90, 2)),
			("GBP", "USD", Quant::new(110, 2)), // Inconsistent cycle
		];
		let segment2 = vec![
			("JPY", "AUD", Quant::new(70, 2)),
			("AUD", "CAD", Quant::new(100, 2)),
			("CAD", "JPY", Quant::new(150, 2)), // Inconsistent cycle
		];
		let segment3 = vec![
			("INR", "CNY", Quant::new(6, 1)),
			("CNY", "CHF", Quant::new(10, 1)),
			("CHF", "INR", Quant::new(50, 1)), // Inconsistent cycle
		];

		// Add rates for each segment
		for (from, to, rate) in segment1 {
			let amount_from = Amount::new(Quant::new(100, 0), from);
			let amount_to = Amount::new(rate, to);
			graph
				.add_rate(&amount_from, &amount_to, true)
				.expect("Could not add rate");
		}
		for (from, to, rate) in segment2 {
			let amount_from = Amount::new(Quant::new(100, 0), from);
			let amount_to = Amount::new(rate, to);
			graph
				.add_rate(&amount_from, &amount_to, true)
				.expect("Could not add rate");
		}
		for (from, to, rate) in segment3 {
			let amount_from = Amount::new(Quant::new(100, 0), from);
			let amount_to = Amount::new(rate, to);
			graph
				.add_rate(&amount_from, &amount_to, true)
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
				let a = Amount::new(Quant::from_i128((i + 1) as i128), seg1[i]);
				let b = Amount::new(Quant::from_i128((j + 1) as i128), seg1[j]);
				graph.add_rate(&a, &b, true).expect("Could not add rate");
			}
		}

		// Segment 2
		let seg2 = ["JPY", "AUD", "CAD"];
		for i in 0..seg2.len() {
			for j in i + 1..seg2.len() {
				let a = Amount::new(Quant::from_i128((i + 1) as i128), seg2[i]);
				let b = Amount::new(Quant::from_i128((j + 1) as i128), seg2[j]);
				graph.add_rate(&a, &b, true).expect("Could not add rate");
			}
		}

		// Segment 3
		let seg3 = ["CHF", "CNY", "INR"];
		for i in 0..seg3.len() {
			for j in i + 1..seg3.len() {
				let a = Amount::new(Quant::from_i128((i + 1) as i128), seg3[i]);
				let b = Amount::new(Quant::from_i128((j + 1) as i128), seg3[j]);
				graph.add_rate(&a, &b, true).expect("Could not add rate");
			}
		}

		// Test for inconsistent cycles
		assert!(
			!graph.has_inconsistent_cycle(),
			"No inconsistent cycles expected"
		);
	}

	#[test]
	fn test_extreme_high_rate() {
		let mut graph = Graph::new();
		let a = Amount::new(Quant::new(1, 0), "USD");
		let b = Amount::new(Quant::new(1_000_000_000_000, 0), "BTC"); // 1 USD = 1 trillion BTC
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let expected_output = Quant::new(1_000_000_000_000, 0);
		let result = graph.convert("USD", "BTC");

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_extreme_low_rate() {
		let mut graph = Graph::new();
		let a = Amount::new(Quant::new(1, 0), "USD");
		let b = Amount::new(Quant::from_frac(1, 1_000_000_000_000), "BTC"); // 1 USD = 1e-12 BTC
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let expected_output = Quant::from_frac(1, 1_000_000_000_000);
		let result = graph.convert("USD", "BTC");

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_combined_extreme_rates() {
		let mut graph = Graph::new();
		let a = Amount::new(Quant::new(1, 0), "USD");
		let b = Amount::new(Quant::new(1_000_000_000, 0), "BTC");
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let c = Amount::new(Quant::from_frac(1, 1_000_000_000), "BTC");
		let d = Amount::new(Quant::new(1, 0), "ETH");
		graph.add_rate(&c, &d, true).expect("Could not add rate");

		let expected_output = Quant::new(1_000_000_000_000_000_000, 0);
		let result = graph.convert("USD", "ETH");

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_edge_case_self_loop() {
		let mut graph = Graph::new();
		let a = Amount::new(Quant::new(1, 0), "USD");
		let b = Amount::new(Quant::new(1, 0), "USD"); // Self-loop
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let result = graph.convert("USD", "USD");
		assert_eq!(result.unwrap(), Quant::from_i128(1));
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_bizarre_fraction_high() {
		let mut graph = Graph::new();
		let a = Amount::new(Quant::from_frac(123456789, 987654321), "USD");
		let b = Amount::new(Quant::from_frac(987654321, 123456789), "EUR");
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let expected_output =
			Quant::from_frac(12042729108518161, 188167638891241);
		let result = graph.convert("USD", "EUR");

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_bizarre_fraction_low() {
		let mut graph = Graph::new();
		let a = Amount::new(Quant::from_frac(1, 987654321), "USD"); // 1/987654321 USD
		let b = Amount::new(Quant::new(1, 0), "JPY"); // 1 JPY
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let expected_output = Quant::from_frac(1, 987654321);
		let result = graph.convert("JPY", "USD");

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_bizarre_fraction_chain() {
		let mut graph = Graph::new();

		// Add several fractions in a chain
		let a = Amount::new(Quant::from_frac(123456789, 987654321), "USD");
		let b = Amount::new(Quant::from_frac(987654321, 123456789), "EUR");
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let c = Amount::new(Quant::from_frac(22222222, 33333333), "EUR");
		let d = Amount::new(Quant::from_frac(33333333, 22222222), "GBP");
		graph.add_rate(&c, &d, true).expect("Could not add rate");

		let expected_output =
			Quant::from_frac(108384561976663449, 752670555564964);
		let result = graph.convert("USD", "GBP");

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_extreme_bizarre_fraction() {
		let mut graph = Graph::new();

		// Extreme fraction rates
		let a = Amount::new(Quant::from_frac(1, 1_000_000_000_007), "BTC");
		let b = Amount::new(Quant::from_frac(1_000_000_000_007, 1), "ETH");
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let expected_output =
			Quant::from_frac(1_000_000_000_014_000_000_000_049, 1);
		let result = graph.convert("BTC", "ETH");

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_bizarre_fraction_cascade() {
		let mut graph = Graph::new();

		// Chain of bizarre fractions
		let a = Amount::new(Quant::from_frac(987654321, 123456789), "USD");
		let b = Amount::new(Quant::from_frac(123456789, 987654321), "EUR");
		graph.add_rate(&a, &b, true).expect("Could not add rate");

		let c = Amount::new(Quant::from_frac(44444444, 55555555), "EUR");
		let d = Amount::new(Quant::from_frac(55555555, 44444444), "JPY");
		graph.add_rate(&c, &d, true).expect("Could not add rate");

		let e = Amount::new(Quant::from_frac(22222222, 33333333), "JPY");
		let f = Amount::new(Quant::from_frac(33333333, 22222222), "AUD");
		graph.add_rate(&e, &f, true).expect("Could not add rate");

		let expected_output =
			Quant::from_frac(42337718750529225, 770734662945162304);
		let result = graph.convert("USD", "AUD");

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}
}
