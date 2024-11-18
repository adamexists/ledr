use std::process::Command;
use std::fs;

// TODO: When writing .build.yml, be sure to not parallelize the tests:
//  i.e. cargo test -- --test-threads=1

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

fn execute(
    subfolder: &str, test_cases: Vec<(&str, &str)>, cmd: &str, args: Vec<&str>,
) {
    for (input_file, expected_output_file) in test_cases {
        println!("running for {}...", input_file);

        let loc = format!(
            "{}/{}/{}",
            "tests/test_data",
            subfolder,
            input_file
        );

        let all_args = [
            vec![
                "run",
                "--",
                "-f",
                loc.as_str(),
                cmd,
            ],
            args.clone(),
        ].concat();

        let output = Command::new("cargo")
            .args(all_args).output()
            .expect("Failed to execute process");

        assert!(output.status.success(), "{} failed processing!", input_file);

        let stdout = String::from_utf8_lossy(&output.stdout);

        let expected_output = fs::read_to_string(
            format!(
                "{}/{}/{}",
                "tests/test_data",
                subfolder,
                expected_output_file
            ),
        ).expect("Failed to read expected output file");

        assert_eq!(stdout.trim(),
                   expected_output.trim(),
                   "Output did not match for {}; expected:\n{}\ngot:\n{}",
                   input_file,
                   expected_output,
                   stdout
        );
    }
}
