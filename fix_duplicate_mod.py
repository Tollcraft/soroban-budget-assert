with open("cargo-budget-report/src/main.rs", "r") as f:
    lines = f.readlines()

new_lines = []
found = False
for line in lines:
    if line.strip() == "mod json_output;":
        if not found:
            found = True
            new_lines.append(line)
    else:
        new_lines.append(line)

with open("cargo-budget-report/src/main.rs", "w") as f:
    f.writelines(new_lines)
