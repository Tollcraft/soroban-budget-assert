use anyhow::{Context, Result};
use base64::{engine::general_purpose, Engine as _};
use ed25519_dalek::Signer;
use rand::RngCore;
use reqwest::blocking::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Cursor;
use std::str::FromStr;
use std::time::Duration;
use stellar_strkey::Strkey;
use stellar_xdr::curr::{
    BytesM, ContractExecutable, ContractIdPreimage, ContractIdPreimageFromAddress,
    CreateContractArgsV2, DecoratedSignature, Hash, HostFunction, InvokeContractArgs,
    InvokeHostFunctionOp, Limited, Limits, MuxedAccount, Operation, Preconditions, ReadXdr,
    ScAddress, ScSymbol, SequenceNumber, Signature, SignatureHint, SorobanTransactionData,
    Transaction, TransactionEnvelope, TransactionExt, TransactionSignaturePayload,
    TransactionSignaturePayloadTaggedTransaction, TransactionV1Envelope, Uint256, VecM, WriteXdr,
};

/// RPC endpoint URLs for supported networks
fn rpc_endpoint_for_network(network: &str) -> &str {
    match network {
        "testnet" => "https://soroban-testnet.stellar.org",
        "futurenet" => "https://rpc-futurenet.stellar.org:443",
        "mainnet" => "https://soroban-mainnet.stellar.org:443",
        "local" => "http://localhost:8000",
        _ => network,
    }
}

/// Network passphrase for supported networks
fn network_passphrase_for_network(network: &str) -> &str {
    match network {
        "testnet" => "Test SDF Network ; September 2015",
        "futurenet" => "Test SDF Future Network ; October 2022",
        "mainnet" => "Public Global Stellar Network ; September 2015",
        "local" => "Standalone Network ; February 2017",
        _ => "Test SDF Network ; September 2015",
    }
}

/// JSON-RPC request structure
#[derive(Serialize)]
struct JsonRpcRequest {
    jsonrpc: &'static str,
    id: u64,
    method: String,
    params: Value,
}

/// JSON-RPC response structure
#[derive(Deserialize)]
struct JsonRpcResponse {
    #[allow(dead_code)]
    jsonrpc: String,
    #[allow(dead_code)]
    id: Value,
    #[serde(default)]
    result: Value,
    #[serde(default)]
    error: Option<JsonRpcError>,
}

/// JSON-RPC error structure
#[derive(Deserialize, Debug)]
struct JsonRpcError {
    code: i32,
    message: String,
}

/// Response from `uploadContractWasm` RPC
#[derive(Deserialize)]
struct UploadContractWasmResponse {
    hash: String,
}

/// Response from `sendTransaction` RPC
#[derive(Deserialize)]
pub struct SendTransactionResponse {
    #[serde(rename = "status")]
    #[allow(dead_code)]
    status: String,
}

/// Stellar RPC client for Soroban operations
pub struct SorobanRpcClient {
    client: Client,
    endpoint: String,
    network_passphrase: String,
}

impl SorobanRpcClient {
    /// Create a new RPC client for the given network
    pub fn new(network: &str) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .build()
            .context("Failed to create HTTP client")?;
        Ok(Self {
            client,
            endpoint: rpc_endpoint_for_network(network).to_string(),
            network_passphrase: network_passphrase_for_network(network).to_string(),
        })
    }

    /// Send a JSON-RPC request
    fn send_request(&self, method: &str, params: Value) -> Result<Value> {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: rand::thread_rng().next_u64(),
            method: method.to_string(),
            params,
        };

        let response = self
            .client
            .post(&self.endpoint)
            .json(&request)
            .send()
            .context("Failed to send RPC request")?;

        let rpc_response: JsonRpcResponse =
            response.json().context("Failed to parse RPC response")?;

        if let Some(error) = rpc_response.error {
            anyhow::bail!("RPC error {}: {}", error.code, error.message);
        }

        Ok(rpc_response.result)
    }

    /// Upload contract WASM
    pub fn upload_contract_wasm(&self, wasm_bytes: &[u8]) -> Result<Hash> {
        let wasm_b64 = general_purpose::STANDARD.encode(wasm_bytes);
        let result = self.send_request(
            "uploadContractWasm",
            serde_json::json!({ "wasm": wasm_b64 }),
        )?;
        let response: UploadContractWasmResponse = serde_json::from_value(result)
            .context("Failed to parse uploadContractWasm response")?;
        Hash::from_str(&response.hash).context("Invalid hash in response")
    }

    /// Simulate a transaction
    pub fn simulate_transaction(&self, envelope_b64: &str) -> Result<Value> {
        self.send_request(
            "simulateTransaction",
            serde_json::json!({ "transaction": envelope_b64 }),
        )
    }

    /// Send a transaction
    pub fn send_transaction(&self, envelope_b64: &str) -> Result<SendTransactionResponse> {
        let result = self.send_request(
            "sendTransaction",
            serde_json::json!({ "transaction": envelope_b64 }),
        )?;
        serde_json::from_value(result).context("Failed to parse sendTransaction response")
    }

    /// Get the network passphrase
    pub fn network_passphrase(&self) -> &str {
        &self.network_passphrase
    }
}

/// Parse a Stellar public key string (G...) to Uint256 (raw 32 bytes)
fn parse_account_id(account_str: &str) -> Result<Uint256> {
    let strkey = Strkey::from_string(account_str).context("Invalid account address")?;
    match strkey {
        Strkey::PublicKeyEd25519(pk) => Ok(Uint256(pk.0)),
        Strkey::MuxedAccountEd25519(ma) => Ok(Uint256(ma.ed25519)),
        _ => anyhow::bail!("Expected Ed25519 public key or muxed account"),
    }
}

/// Parse a contract ID string (C...) to Hash
fn parse_contract_id(contract_str: &str) -> Result<Hash> {
    let strkey = Strkey::from_string(contract_str).context("Invalid contract ID")?;
    match strkey {
        Strkey::Contract(c) => Ok(Hash(c.0)),
        _ => anyhow::bail!("Expected contract address"),
    }
}

/// Compute contract ID from CreateContractV2 parameters
/// The contract ID is the SHA-256 hash of the contract ID preimage
fn compute_contract_id(_wasm_hash: &Hash, source_account: &str, salt: &Uint256) -> Result<String> {
    let source_account_id = parse_account_id(source_account)?;
    let public_key = stellar_xdr::curr::PublicKey::PublicKeyTypeEd25519(source_account_id);
    let account_id = stellar_xdr::curr::AccountId(public_key);

    // Create the contract ID preimage (same as the network computes it)
    let preimage = ContractIdPreimage::Address(ContractIdPreimageFromAddress {
        address: ScAddress::Account(account_id),
        salt: salt.clone(),
    });

    // The contract ID is SHA-256(preimage) where preimage is the XDR encoding
    let mut cursor = Cursor::new(Vec::new());
    let mut limited = Limited::new(&mut cursor, Limits::none());
    preimage
        .write_xdr(&mut limited)
        .context("Failed to encode contract ID preimage")?;

    let preimage_bytes = cursor.into_inner();
    let hash = Sha256::digest(&preimage_bytes);

    // Convert hash to string (C... format) using Strkey
    let contract_strkey = Strkey::Contract(stellar_strkey::Contract(hash.into()));
    Ok(contract_strkey.to_string())
}

/// Build a transaction envelope for contract deployment
pub fn build_deploy_contract_transaction(
    _client: &SorobanRpcClient,
    wasm_hash: &Hash,
    source_account: &str,
    salt: &Uint256,
    constructor_args: Vec<stellar_xdr::curr::ScVal>,
) -> Result<TransactionEnvelope> {
    // Parse source account
    let source_account_id = parse_account_id(source_account)?;
    let public_key = stellar_xdr::curr::PublicKey::PublicKeyTypeEd25519(source_account_id.clone());
    let account_id = stellar_xdr::curr::AccountId(public_key);
    let source = MuxedAccount::Ed25519(source_account_id);

    // Create contract ID preimage
    let contract_id_preimage = ContractIdPreimage::Address(ContractIdPreimageFromAddress {
        address: ScAddress::Account(account_id),
        salt: salt.clone(),
    });

    // Build create contract args
    let create_args = CreateContractArgsV2 {
        contract_id_preimage,
        executable: ContractExecutable::Wasm(wasm_hash.clone()),
        constructor_args: VecM::try_from(constructor_args).unwrap_or_default(),
    };

    // Build host function
    let host_function = HostFunction::CreateContractV2(create_args);

    // Build invoke host function operation
    let invoke_op = InvokeHostFunctionOp {
        host_function,
        auth: VecM::default(),
    };

    // Build operation
    let operation = Operation {
        source_account: None,
        body: stellar_xdr::curr::OperationBody::InvokeHostFunction(invoke_op),
    };

    // Build transaction
    let transaction = Transaction {
        source_account: source,
        fee: 1_000_000,             // 1 XLM max fee
        seq_num: SequenceNumber(0), // Will be set by simulation
        cond: Preconditions::None,
        memo: stellar_xdr::curr::Memo::None,
        operations: VecM::try_from(vec![operation]).unwrap(),
        ext: TransactionExt::V0,
    };

    // Build envelope
    let envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx: transaction,
        signatures: VecM::default(),
    });

    Ok(envelope)
}

/// Build a transaction envelope for contract invocation
pub fn build_invoke_contract_transaction(
    _client: &SorobanRpcClient,
    contract_id: &str,
    source_account: &str,
    function_name: &str,
    args: Vec<stellar_xdr::curr::ScVal>,
) -> Result<TransactionEnvelope> {
    let source_account_id = parse_account_id(source_account)?;
    let source = MuxedAccount::Ed25519(source_account_id);
    let contract_address = ScAddress::Contract(parse_contract_id(contract_id)?);

    let invoke_args = InvokeContractArgs {
        contract_address,
        function_name: ScSymbol(
            function_name
                .to_string()
                .try_into()
                .context("Function name too long")?,
        ),
        args: VecM::try_from(args).unwrap_or_default(),
    };

    let host_function = HostFunction::InvokeContract(invoke_args);

    let invoke_op = InvokeHostFunctionOp {
        host_function,
        auth: VecM::default(),
    };

    let operation = Operation {
        source_account: None,
        body: stellar_xdr::curr::OperationBody::InvokeHostFunction(invoke_op),
    };

    let transaction = Transaction {
        source_account: source,
        fee: 1_000_000,
        seq_num: SequenceNumber(0),
        cond: Preconditions::None,
        memo: stellar_xdr::curr::Memo::None,
        operations: VecM::try_from(vec![operation]).unwrap(),
        ext: TransactionExt::V0,
    };

    let envelope = TransactionEnvelope::Tx(TransactionV1Envelope {
        tx: transaction,
        signatures: VecM::default(),
    });

    Ok(envelope)
}

/// Encode a transaction envelope to base64 XDR
pub fn envelope_to_base64(envelope: &TransactionEnvelope) -> Result<String> {
    let mut cursor = Cursor::new(Vec::new());
    let mut limited = Limited::new(&mut cursor, Limits::none());
    envelope
        .write_xdr(&mut limited)
        .context("Failed to encode envelope to XDR")?;
    Ok(general_purpose::STANDARD.encode(cursor.into_inner()))
}

/// Sign a transaction envelope with the given secret key and return the
/// signed envelope.
///
/// The secret key should be a Stellar secret key string (S...).
/// This function computes the network ID hash, creates the
/// `TransactionSignaturePayload`, signs it, and attaches the
/// `DecoratedSignature` to the envelope.
pub fn sign_transaction_envelope(
    envelope: &mut TransactionEnvelope,
    secret_key: &str,
    network_passphrase: &str,
) -> Result<()> {
    use sha2::Digest;

    // Decode secret key
    let strkey = Strkey::from_string(secret_key).context("Invalid secret key")?;
    let seed = match strkey {
        Strkey::PrivateKeyEd25519(pk) => pk.0,
        _ => anyhow::bail!("Expected Ed25519 private key (S...)"),
    };

    // Create signing key from seed
    let signing_key = ed25519_dalek::SigningKey::from_bytes(&seed);

    // Compute network ID hash
    let network_hash = Sha256::digest(network_passphrase.as_bytes());
    let network_id = Hash(network_hash.into());

    match envelope {
        TransactionEnvelope::Tx(ref mut tx_env) => {
            let payload = TransactionSignaturePayload {
                network_id,
                tagged_transaction: TransactionSignaturePayloadTaggedTransaction::Tx(
                    tx_env.tx.clone(),
                ),
            };

            let mut cursor = Cursor::new(Vec::new());
            let mut limited = Limited::new(&mut cursor, Limits::none());
            payload
                .write_xdr(&mut limited)
                .context("Failed to encode signature payload")?;

            let hash = Sha256::digest(cursor.into_inner());
            let signature = signing_key.sign(&hash);
            let signature_bytes: [u8; 64] = signature.to_bytes();

            let hint_bytes: [u8; 4] = signing_key.verifying_key().to_bytes()[28..32]
                .try_into()
                .unwrap();
            let decorated = DecoratedSignature {
                hint: SignatureHint(hint_bytes),
                signature: Signature(
                    BytesM::try_from(&signature_bytes[..])
                        .map_err(|_| anyhow::anyhow!("Signature too long"))?,
                ),
            };

            tx_env.signatures = VecM::try_from(vec![decorated])
                .map_err(|_| anyhow::anyhow!("Too many signatures"))?;
        }
        _ => anyhow::bail!("Unsupported envelope type"),
    }

    Ok(())
}

/// Deploy a contract using the RPC client.
///
/// Uploads the WASM to the RPC node and computes the contract ID locally
/// from the WASM hash, source account, and a random salt. No deployment
/// transaction is submitted — the contract ID is deterministic and can be
/// computed before the contract exists on-ledger. This is sufficient for
/// the simulation-only workflow that `cargo-budget-report` uses.
///
/// If a `secret_key` is provided, the deployment transaction is signed
/// and submitted to the network so subsequent `simulateTransaction` calls
/// for invocations have a real contract to execute against.
pub fn deploy_contract_via_rpc(
    client: &SorobanRpcClient,
    wasm_bytes: &[u8],
    source_account: &str,
    _package_name: &str,
    secret_key: Option<&str>,
) -> Result<String> {
    // Upload WASM
    let wasm_hash = client
        .upload_contract_wasm(wasm_bytes)
        .context("Failed to upload contract WASM")?;

    // Generate salt
    let mut salt = Uint256([0u8; 32]);
    rand::thread_rng().fill_bytes(&mut salt.0);

    // Compute contract ID from preimage (same way the network does it)
    let contract_id = compute_contract_id(&wasm_hash, source_account, &salt)
        .context("Failed to compute contract ID")?;

    // If a secret key is provided, build, sign, and submit the deployment
    // transaction so the contract actually exists on the ledger for
    // subsequent invoke simulations.
    if let Some(sk) = secret_key {
        let mut envelope =
            build_deploy_contract_transaction(client, &wasm_hash, source_account, &salt, vec![])
                .context("Failed to build deployment transaction")?;

        sign_transaction_envelope(&mut envelope, sk, client.network_passphrase())?;

        let envelope_b64 = envelope_to_base64(&envelope)?;

        // Simulate first to get resource fee
        let sim_result = client
            .simulate_transaction(&envelope_b64)
            .context("Failed to simulate deployment transaction")?;

        let tx_data_b64 = sim_result["transactionData"]
            .as_str()
            .context("No transactionData in simulation response")?;

        let tx_data = SorobanTransactionData::from_xdr_base64(tx_data_b64, Limits::none())
            .context("Failed to decode SorobanTransactionData")?;

        // Rebuild with correct fee and sign
        let mut envelope =
            build_deploy_contract_transaction(client, &wasm_hash, source_account, &salt, vec![])
                .context("Failed to rebuild deployment transaction")?;

        if let TransactionEnvelope::Tx(ref mut tx_env) = envelope {
            tx_env.tx.fee = tx_data.resource_fee.max(100_000) as u32;
        }

        sign_transaction_envelope(&mut envelope, sk, client.network_passphrase())?;

        let envelope_b64 = envelope_to_base64(&envelope)?;

        let _send_result = client
            .send_transaction(&envelope_b64)
            .context("Failed to send deployment transaction")?;
    }

    Ok(contract_id)
}

/// Simulate a contract function call using RPC
pub fn simulate_contract_function(
    client: &SorobanRpcClient,
    contract_id: &str,
    source_account: &str,
    function_name: &str,
    args: Vec<stellar_xdr::curr::ScVal>,
) -> Result<(u32, u32, u32)> {
    let envelope =
        build_invoke_contract_transaction(client, contract_id, source_account, function_name, args)
            .context("Failed to build invoke transaction")?;

    let envelope_b64 = envelope_to_base64(&envelope)?;

    let sim_result = client
        .simulate_transaction(&envelope_b64)
        .context("Failed to simulate transaction")?;

    extract_metrics_from_response(&sim_result)
}

/// Extract metrics from simulateTransaction response.
///
/// The `response` should be the JSON-RPC `result` object (as returned by
/// [`SorobanRpcClient::simulate_transaction`]), which contains the
/// `transactionData` field directly.
pub fn extract_metrics_from_response(response: &Value) -> Result<(u32, u32, u32)> {
    if let Some(error) = response.get("error") {
        anyhow::bail!("RPC error: {}", error);
    }

    let tx_data_b64 = response["transactionData"]
        .as_str()
        .context("No transactionData in simulation response")?;

    let tx_data = SorobanTransactionData::from_xdr_base64(tx_data_b64, Limits::none())
        .context("Failed to decode SorobanTransactionData")?;

    Ok((
        tx_data.resources.instructions,
        tx_data.resources.read_bytes,
        tx_data.resources.write_bytes,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rpc_endpoint_for_network() {
        assert_eq!(
            rpc_endpoint_for_network("testnet"),
            "https://soroban-testnet.stellar.org"
        );
        assert_eq!(
            rpc_endpoint_for_network("futurenet"),
            "https://rpc-futurenet.stellar.org:443"
        );
        assert_eq!(
            rpc_endpoint_for_network("mainnet"),
            "https://soroban-mainnet.stellar.org:443"
        );
        assert_eq!(rpc_endpoint_for_network("local"), "http://localhost:8000");
        assert_eq!(rpc_endpoint_for_network("custom"), "custom");
    }

    #[test]
    fn test_network_passphrase_for_network() {
        assert_eq!(
            network_passphrase_for_network("testnet"),
            "Test SDF Network ; September 2015"
        );
        assert_eq!(
            network_passphrase_for_network("futurenet"),
            "Test SDF Future Network ; October 2022"
        );
        assert_eq!(
            network_passphrase_for_network("mainnet"),
            "Public Global Stellar Network ; September 2015"
        );
        assert_eq!(
            network_passphrase_for_network("local"),
            "Standalone Network ; February 2017"
        );
    }

    #[test]
    fn test_json_rpc_request_serialization() {
        let request = JsonRpcRequest {
            jsonrpc: "2.0",
            id: 1,
            method: "testMethod".to_string(),
            params: serde_json::json!({"key": "value"}),
        };
        let json = serde_json::to_string(&request).unwrap();
        assert!(json.contains("testMethod"));
        assert!(json.contains("key"));
    }

    #[test]
    fn test_parse_account_id() {
        // Test with a valid Ed25519 public key (G...)
        let account = "GAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAWHF";
        let result = parse_account_id(account);
        assert!(result.is_ok());
    }

    #[test]
    fn test_parse_contract_id() {
        // Generate a valid contract ID from the zero hash
        let hash = stellar_strkey::Contract([0u8; 32]);
        let contract = stellar_strkey::Strkey::Contract(hash).to_string();
        let result = parse_contract_id(&contract);
        assert!(
            result.is_ok(),
            "Failed to parse valid contract ID '{}': {:?}",
            contract,
            result.err()
        );
    }
}
