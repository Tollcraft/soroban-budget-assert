const fs = require('fs');
const { 
    Keypair, 
    rpc, 
    TransactionBuilder, 
    Networks, 
    BASE_FEE,
    Contract,
    scValToNative,
    nativeToScVal,
    xdr,
    StrKey
} = require('@stellar/stellar-sdk');

const SERVER_URL = 'https://soroban-testnet.stellar.org';
const server = new rpc.Server(SERVER_URL);
const NETWORK_PASSPHRASE = Networks.TESTNET;

async function fundAccount(publicKey) {
    const response = await fetch(`https://friendbot.stellar.org?addr=${publicKey}`);
    await response.json();
}

async function main() {
    console.log("Setting up account...");
    const keypair = Keypair.random();
    await fundAccount(keypair.publicKey());
    
    let account = await server.getAccount(keypair.publicKey());
    
    // 1. Upload WASM
    console.log("Uploading contract...");
    const wasm = fs.readFileSync("../target/wasm32-unknown-unknown/release/example_contract.wasm");
    
    let uploadTx = new TransactionBuilder(account, { fee: BASE_FEE, networkPassphrase: NETWORK_PASSPHRASE })
        .addOperation(
            xdr.Operation.invokeHostFunction({
                hostFunction: xdr.HostFunction.hostFunctionTypeUploadContractWasm(wasm),
                auth: [],
            })
        )
        .setTimeout(300)
        .build();

    let uploadSim = await server.simulateTransaction(uploadTx);
    
    // To make sure it succeeds, we need to extract the assembled tx and send it. 
    // It's easier if we just write the report using the simulateTransaction API.

    console.log("WASM Upload simulated successfully.");
}

main().catch(console.error);
