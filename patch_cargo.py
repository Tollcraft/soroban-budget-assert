with open("amm-pool-contract/Cargo.toml", "r") as f:
    text = f.read()

text = text.replace("sdk20 = []\n", "sdk20 = []\nsdk22 = []\n")

with open("amm-pool-contract/Cargo.toml", "w") as f:
    f.write(text)
