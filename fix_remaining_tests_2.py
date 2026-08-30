with open("cargo-budget-report/src/edge_case_tests.rs", "r") as f:
    text = f.read()

text = text.replace(
    '&["--to".into(), "GBP".into(), "--amount".into(), "100".into()],\n        );',
    '&["--to".into(), "GBP".into(), "--amount".into(), "100".into()],\n            None\n        );'
)

with open("cargo-budget-report/src/edge_case_tests.rs", "w") as f:
    f.write(text)


with open("cargo-budget-report/src/additional_edge_tests.rs", "r") as f:
    text = f.read()

text = text.replace(
    '&["--value".into(), "!@#$%^&*()".into()],\n        );',
    '&["--value".into(), "!@#$%^&*()".into()],\n            None\n        );'
)
text = text.replace('csv::Writer::from_writer(vec![], None)', 'csv::Writer::from_writer(vec![])')

with open("cargo-budget-report/src/additional_edge_tests.rs", "w") as f:
    f.write(text)
