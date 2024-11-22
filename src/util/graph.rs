use crate::util::amount::Amount;
use crate::util::scalar::Scalar;
use anyhow::{bail, Error};
use std::collections::{HashMap, HashSet};

pub struct Graph {
	nodes: HashMap<String, Node>, // currency symbol -> its Node
}

struct Node {
	edges: HashMap<String, Scalar>, // currency symbol -> conversion rate
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
	) -> Result<(), Error> {
		let rate_ab = a.value / b.value;
		let rate_ba = b.value / a.value;

		if rate_ab == Scalar::zero() || rate_ba == Scalar::zero() {
			bail!("Exchange rates cannot be zero");
		}

		// Update the graph
		self.nodes
			.entry(a.currency.clone())
			.or_insert_with(Node::new)
			.edges
			.insert(b.currency.clone(), rate_ab);

		self.nodes
			.entry(b.currency.clone())
			.or_insert_with(Node::new)
			.edges
			.insert(a.currency.clone(), rate_ba);

		Ok(())
	}

	fn has_inconsistent_cycle(&self) -> bool {
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
		println!(
                "Visiting: {}, rate_product: {}, start: {}, visited: {:?}, rec_stack: {:?}",
                current, rate_product, start, visited, rec_stack
        );

		if rec_stack.contains(current) {
			// Cycle detected, but only check consistency if we're back at the starting node
			if current == start {
				println!(
                                "Cycle completed back to start: {}, accumulated rate_product: {}",
                                start, rate_product
                        );
				return rate_product < 0.95
					|| rate_product > 1.05;
			} else {
				println!(
                                "Cycle detected involving node: {}, but not back to start. Skipping...",
                                current
                        );
				return false;
			}
		}

		if visited.contains(current) {
			// Already visited and not in the current recursion path
			println!(
				"Already visited node: {}. Skipping...",
				current
			);
			return false;
		}

		// Mark the current node as visited and add to the recursion stack
		visited.insert(current.to_string());
		rec_stack.insert(current.to_string());

		if let Some(node) = self.nodes.get(current) {
			for (neighbor, rate) in &node.edges {
				let new_rate_product =
					rate_product * rate.to_f64();
				println!(
                                "Traversing edge: {} -> {}, rate: {}, new_rate_product: {}",
                                current, neighbor, rate, new_rate_product
                        );

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
		println!("Backtracking from: {}", current);

		false
	}

	/// Converts between two currencies, preserving precision
	pub fn convert(
		&self,
		base: &str,
		quote: &str,
	) -> Result<Scalar, Error> {
		if base == quote {
			return Ok(Scalar::from_i128(1));
		}

		let mut visited = HashMap::new();
		let mut queue = vec![(base.to_string(), Scalar::from_i128(1))];

		while let Some((current_currency, current_rate)) = queue.pop() {
			if let Some(node) = self.nodes.get(&current_currency) {
				for (neighbor, rate) in &node.edges {
					if !visited.contains_key(neighbor) {
						let new_rate =
							current_rate * *rate;

						if neighbor == quote {
							return Ok(new_rate);
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

		bail!("Conversion path not found");
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
		graph.add_rate(&a, &b).expect("Could not add rate");

		let expected_output = Scalar::new(2, 0);
		let result = graph.convert("USD", "EUR");

		assert!(result.is_ok());
		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_reverse_conversion() {
		let mut graph = Graph::new();
		let a = Amount::new(Scalar::new(2, 0), "USD".to_string());
		let b = Amount::new(Scalar::new(1, 0), "EUR".to_string());
		graph.add_rate(&a, &b).expect("Could not add rate");

		let expected_output = Scalar::new(5, 1);
		let result = graph.convert("EUR", "USD");

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_indirect_conversion() {
		let mut graph = Graph::new();
		let a = Amount::new(Scalar::new(2, 0), "USD".to_string());
		let b = Amount::new(Scalar::new(1, 0), "EUR".to_string());
		graph.add_rate(&a, &b).expect("Could not add rate");

		let c = Amount::new(Scalar::new(5, 0), "EUR".to_string());
		let d = Amount::new(Scalar::new(1, 0), "GBP".to_string());
		graph.add_rate(&c, &d).expect("Could not add rate");

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
		graph.add_rate(&a, &b).expect("Could not add rate");

		let c = Amount::new(Scalar::new(5, 0), "EUR".to_string());
		let d = Amount::new(Scalar::new(1, 0), "GBP".to_string());
		graph.add_rate(&c, &d).expect("Could not add rate");

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
		graph.add_rate(&a, &b).expect("Could not add rate");

		let c = Amount::new(Scalar::new(3, 0), "JPY".to_string());
		let d = Amount::new(Scalar::new(1, 0), "INR".to_string());
		graph.add_rate(&c, &d).expect("Could not add rate");

		let result = graph.convert("USD", "JPY");

		assert!(result.is_err());
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_large_graph_conversion() {
		let mut graph = Graph::new();
		let a = Amount::new(Scalar::new(2, 0), "USD".to_string());
		let b = Amount::new(Scalar::new(1, 0), "EUR".to_string());
		graph.add_rate(&a, &b).expect("Could not add rate");

		let c = Amount::new(Scalar::new(5, 0), "EUR".to_string());
		let d = Amount::new(Scalar::new(1, 0), "GBP".to_string());
		graph.add_rate(&c, &d).expect("Could not add rate");

		let e = Amount::new(Scalar::new(10, 0), "GBP".to_string());
		let f = Amount::new(Scalar::new(1, 0), "JPY".to_string());
		graph.add_rate(&e, &f).expect("Could not add rate");

		let g = Amount::new(Scalar::new(3, 0), "JPY".to_string());
		let h = Amount::new(Scalar::new(1, 0), "INR".to_string());
		graph.add_rate(&g, &h).expect("Could not add rate");

		let expected_output = Scalar::new(300, 0);
		let result = graph.convert("USD", "INR");

		assert_eq!(result.unwrap(), expected_output);
		assert!(!graph.has_inconsistent_cycle());
	}

	#[test]
	fn test_nonexistent_currency() {
		let graph = Graph::new();

		let result = graph.convert("USD", "XYZ");

		assert!(result.is_err());
	}

	#[test]
	fn test_inconsistent_cycle_detection() {
		let mut graph = Graph::new();

		// Add USD <-> EUR rate
		let a = Amount::new(Scalar::new(2, 0), "USD".to_string());
		let b = Amount::new(Scalar::new(1, 0), "EUR".to_string());
		graph.add_rate(&a, &b).expect("Could not add rate");

		// Add EUR <-> GBP rate
		let c = Amount::new(Scalar::new(5, 0), "EUR".to_string());
		let d = Amount::new(Scalar::new(1, 0), "GBP".to_string());
		graph.add_rate(&c, &d).expect("Could not add rate");

		// Add GBP <-> USD rate that creates an inconsistent cycle
		let e = Amount::new(Scalar::new(1, 0), "GBP".to_string());
		let f = Amount::new(Scalar::new(1, 0), "USD".to_string());
		let result = graph.add_rate(&e, &f);

		// Assert that the inconsistent cycle is detected
		assert!(graph.has_inconsistent_cycle());
	}

	// TODO: Address this test.
	// #[test]
	// fn test_large_connected_graph_no_cycles() {
	// 	let mut graph = Graph::new();
	//
	// 	Construct a large graph with consistent rates
	// let currencies = vec![
	// 	"USD", "EUR", "GBP", "JPY", "AUD", "CAD", "CHF", "CNY",
	// 	"INR", "SGD", "ZAR", "KRW", "BRL", "MXN", "RUB", "HKD",
	// ];
	//
	// Add consistent bidirectional rates
	// for i in 0..currencies.len() {
	// 	for j in i + 1..currencies.len() {
	// 		let rate1 = Scalar::from_i128((i + 1) as i128);
	// 		let rate2 = Scalar::from_i128((j + 1) as i128);
	// 		let a = Amount::new(
	// 			rate1,
	// 			currencies[i].to_string(),
	// 		);
	// 		let b = Amount::new(
	// 			rate2,
	// 			currencies[j].to_string(),
	// 		);
	// 		graph.add_rate(&a, &b)
	// 			.expect("Could not add rate");
	// 	}
	// }
	//
	// Test for inconsistent cycles
	// assert!(
	// 	!graph.has_inconsistent_cycle(),
	// 	"No inconsistent cycles expected"
	// );
	// }

	#[test]
	fn test_large_connected_graph_with_contextual_inconsistencies() {
		let mut graph = Graph::new();

		// Define currencies
		let currencies = vec![
			"USD", "EUR", "GBP", "JPY", "AUD", "CAD", "CHF", "CNY",
			"INR", "SGD", "ZAR", "KRW", "BRL", "MXN", "RUB", "HKD",
		];

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
			graph.add_rate(&amount_from, &amount_to)
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
			graph.add_rate(&amount_from, &amount_to)
				.expect("Could not add rate");
		}
		for (from, to, rate) in segment2 {
			let amount_from = Amount::new(
				Scalar::new(100, 0),
				from.to_string(),
			);
			let amount_to = Amount::new(rate, to.to_string());
			graph.add_rate(&amount_from, &amount_to)
				.expect("Could not add rate");
		}
		for (from, to, rate) in segment3 {
			let amount_from = Amount::new(
				Scalar::new(100, 0),
				from.to_string(),
			);
			let amount_to = Amount::new(rate, to.to_string());
			graph.add_rate(&amount_from, &amount_to)
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
		let seg1 = vec!["USD", "EUR", "GBP"];
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
				graph.add_rate(&a, &b)
					.expect("Could not add rate");
			}
		}

		// Segment 2
		let seg2 = vec!["JPY", "AUD", "CAD"];
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
				graph.add_rate(&a, &b)
					.expect("Could not add rate");
			}
		}

		// Segment 3
		let seg3 = vec!["CHF", "CNY", "INR"];
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
				graph.add_rate(&a, &b)
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
