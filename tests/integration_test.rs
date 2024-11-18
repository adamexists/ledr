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

use std::fs;
use std::process::Command;

#[test]
fn test_integration_no_arguments() {
    let test_cases = vec![
        ("1_in.txt", "1_out.txt"),
        ("2_in.txt", "2_out.txt"),
        ("3_in.txt", "3_out.txt"),
        ("4_in.txt", "4_out.txt"),
        ("5_in.txt", "5_out.txt"),
        ("6_in.txt", "6_out.txt"),
        ("7_in.txt", "7_out.txt"),
        ("8_in.txt", "8_out.txt"),
        ("9_in.txt", "9_out.txt"),
        ("10_in.txt", "10_out.txt"),
    ];

    execute("standard", test_cases, "tb", vec![])
}

#[test]
fn test_integration_collapse_currency() {
    let test_cases = vec![
        ("1_in.txt", "1_out.txt"),
        ("2_in.txt", "2_out.txt"),
        ("3_in.txt", "3_out.txt"),
        ("4_in.txt", "4_out.txt"),
        ("5_in.txt", "5_out.txt"),
    ];

    execute("collapse", test_cases, "tb", vec!["-c", "USD"])
}

#[test]
fn test_integration_max_depth() {
    let test_cases = vec![("1_in.txt", "1_out.txt"), ("2_in.txt", "2_out.txt")];

    execute("maxdepth", test_cases, "bs", vec!["-d", "2"])
}

fn execute(subfolder: &str, test_cases: Vec<(&str, &str)>, cmd: &str, args: Vec<&str>) {
    for (input_file, expected_output_file) in test_cases {
        println!("running for {}...", input_file);

        let loc = format!("{}/{}/{}", "tests/test_data", subfolder, input_file);

        let all_args = [vec!["run", "--", "-f", loc.as_str(), cmd], args.clone()].concat();

        let output = Command::new("cargo")
            .args(all_args)
            .output()
            .expect("Failed to execute process");

        assert!(output.status.success(), "{} failed processing!", input_file);

        let stdout = String::from_utf8_lossy(&output.stdout);

        let expected_output = fs::read_to_string(format!(
            "{}/{}/{}",
            "tests/test_data", subfolder, expected_output_file
        ))
        .expect("Failed to read expected output file");

        assert_eq!(
            stdout.trim(),
            expected_output.trim(),
            "Output did not match for {}; expected:\n{}\ngot:\n{}",
            input_file,
            expected_output,
            stdout
        );
    }
}
