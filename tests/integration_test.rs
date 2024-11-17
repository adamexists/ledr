use std::process::Command;
use std::fs;

#[test]
fn test_against_expected_output() {
    // Define your test cases
    let test_cases = vec![
        ("1_in.txt", "1_out.txt"),
        // ("2_in.txt", "2_out.txt"),
        // ("3_in.txt", "3_out.txt"),
        // ("4_in.txt", "4_out.txt"),
        // ("5_in.txt", "5_out.txt"),
    ];

    for (input_file, expected_output_file) in test_cases {
        // Build and run your project
        let output = Command::new("cargo")
            .arg("run")
            .arg("tb")
            .arg("-f")
            .arg(format!("{}{}", "tests/test_data/", input_file))
            .output()
            .expect("Failed to execute process");

        // Ensure the process ran successfully
        assert!(output.status.success());

        // Capture the output as a string
        let stdout = String::from_utf8_lossy(&output.stdout);

        // Load the expected output
        let expected_output = fs::read_to_string(format!("{}{}", "tests/test_data/", expected_output_file))
            .expect("Failed to read expected output file");

        // Compare the output
        assert_eq!(stdout.trim(), expected_output.trim(), "Output did not match for {}; expected:\n{}\ngot:\n{}", input_file, expected_output, stdout);
    }
}
