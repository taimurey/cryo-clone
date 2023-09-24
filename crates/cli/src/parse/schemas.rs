use std::collections::{HashMap, HashSet};

use cryo_freeze::{ColumnEncoding, Datatype, FileFormat, ParseError, Table};

use super::file_output;
use crate::args::Args;
use cryo_freeze::U256Type;

fn parse_datatypes(raw_inputs: &Vec<String>) -> Result<Vec<Datatype>, ParseError> {
    let mut datatypes = Vec::new();

    for raw_input in raw_inputs {
        match raw_input.as_str() {
            "state_diffs" => {
                datatypes.push(Datatype::BalanceDiffs);
                datatypes.push(Datatype::CodeDiffs);
                datatypes.push(Datatype::NonceDiffs);
                datatypes.push(Datatype::StorageDiffs);
            }
            datatype => {
                let datatype = match datatype {
                    "balance_diffs" => Datatype::BalanceDiffs,
                    "balances" => Datatype::Balances,
                    "blocks" => Datatype::Blocks,
                    "codes" => Datatype::Codes,
                    "code_diffs" => Datatype::CodeDiffs,
                    "contracts" => Datatype::Contracts,
                    "erc20_balances" => Datatype::Erc20Balances,
                    "erc20_metadata" => Datatype::Erc20Metadata,
                    "erc20_supplies" => Datatype::Erc20Supplies,
                    "erc20_transfers" => Datatype::Erc20Transfers,
                    "erc721_metadata" => Datatype::Erc721Metadata,
                    "erc721_transfers" => Datatype::Erc721Transfers,
                    "eth_calls" => Datatype::EthCalls,
                    "events" => Datatype::Logs,
                    "logs" => Datatype::Logs,
                    "native_transfers" => Datatype::NativeTransfers,
                    "nonce_diffs" => Datatype::NonceDiffs,
                    "nonces" => Datatype::Nonces,
                    "opcode_traces" => Datatype::VmTraces,
                    "storage_diffs" => Datatype::StorageDiffs,
                    "storages" => Datatype::Storages,
                    "trace_calls" => Datatype::TraceCalls,
                    "traces" => Datatype::Traces,
                    "transactions" => Datatype::Transactions,
                    "transaction_addresses" => Datatype::TransactionAddresses,
                    "txs" => Datatype::Transactions,
                    "vm_traces" => Datatype::VmTraces,
                    _ => {
                        let name = format!("invalid datatype {}", datatype);
                        return Err(ParseError::ParseError(name))
                    }
                };
                datatypes.push(datatype)
            }
        }
    }
    Ok(datatypes)
}

pub(crate) fn parse_schemas(args: &Args) -> Result<HashMap<Datatype, Table>, ParseError> {
    let datatypes = parse_datatypes(&args.datatype)?;
    let output_format = file_output::parse_output_format(args)?;

    let u256_types = if let Some(raw_u256_types) = &args.u256_types {
        let mut u256_types: HashSet<U256Type> = HashSet::new();
        for raw in raw_u256_types.iter() {
            let u256_type = match raw.to_lowercase() {
                raw if raw == "binary" => U256Type::Binary,
                raw if raw == "string" => U256Type::String,
                raw if raw == "str" => U256Type::String,
                raw if raw == "f32" => U256Type::F32,
                raw if raw == "float32" => U256Type::F32,
                raw if raw == "f64" => U256Type::F64,
                raw if raw == "float64" => U256Type::F64,
                raw if raw == "float" => U256Type::F64,
                raw if raw == "u32" => U256Type::U32,
                raw if raw == "uint32" => U256Type::U32,
                raw if raw == "u64" => U256Type::U64,
                raw if raw == "uint64" => U256Type::U64,
                raw if raw == "decimal128" => U256Type::Decimal128,
                raw if raw == "d128" => U256Type::Decimal128,
                _ => return Err(ParseError::ParseError("bad u256 type".to_string())),
            };
            u256_types.insert(u256_type);
        }
        u256_types
    } else {
        HashSet::from_iter(vec![U256Type::Binary, U256Type::String, U256Type::F64])
    };
    let binary_column_format = match args.hex | (output_format != FileFormat::Parquet) {
        true => ColumnEncoding::Hex,
        false => ColumnEncoding::Binary,
    };

    let sort = parse_sort(&args.sort, &datatypes)?;
    let schemas: Result<HashMap<Datatype, Table>, ParseError> = datatypes
        .iter()
        .map(|datatype| {
            datatype
                .table_schema(
                    &u256_types,
                    &binary_column_format,
                    &args.include_columns,
                    &args.exclude_columns,
                    &args.columns,
                    sort[datatype].clone(),
                    None,
                )
                .map(|schema| (*datatype, schema))
                .map_err(|e| {
                    ParseError::ParseError(format!(
                        "Failed to get schema for datatype: {:?}, {:?}",
                        datatype, e
                    ))
                })
        })
        .collect();

    // make sure all included columns ended up in at least one schema
    if let (Ok(schemas), Some(include_columns)) = (&schemas, &args.include_columns) {
        let mut unknown_columns = Vec::new();
        for column in include_columns.iter() {
            let mut in_a_schema = false;

            for schema in schemas.values() {
                if schema.has_column(column) {
                    in_a_schema = true;
                    break
                }
            }

            if !in_a_schema && column != "all" {
                unknown_columns.push(column);
            }
        }
        if !unknown_columns.is_empty() {
            return Err(ParseError::ParseError(format!(
                "datatypes do not support these columns: {:?}",
                unknown_columns
            )))
        }
    };

    // make sure all excluded columns are excluded from at least one schema
    if let (Ok(schemas), Some(exclude_columns)) = (&schemas, &args.exclude_columns) {
        let mut unknown_columns = Vec::new();
        for column in exclude_columns.iter() {
            let mut in_a_schema = false;

            for datatype in schemas.keys() {
                if datatype.dataset().column_types().contains_key(&column.as_str()) {
                    in_a_schema = true;
                    break
                }
            }

            if !in_a_schema {
                unknown_columns.push(column);
            }
        }
        if !unknown_columns.is_empty() {
            return Err(ParseError::ParseError(format!(
                "datatypes do not support these columns: {:?}",
                unknown_columns
            )))
        }
    };

    schemas
}

fn parse_sort(
    raw_sort: &Option<Vec<String>>,
    datatypes: &Vec<Datatype>,
) -> Result<HashMap<Datatype, Option<Vec<String>>>, ParseError> {
    match raw_sort {
        None => Ok(HashMap::from_iter(
            datatypes.iter().map(|datatype| (*datatype, Some(datatype.dataset().default_sort()))),
        )),
        Some(raw_sort) => {
            if (raw_sort.len() == 1) && (raw_sort[0] == "none") {
                Ok(HashMap::from_iter(datatypes.iter().map(|datatype| (*datatype, None))))
            } else if raw_sort.is_empty() {
                Err(ParseError::ParseError(
                    "must specify columns to sort by, use `none` to disable sorting".to_string(),
                ))
            } else if datatypes.len() > 1 {
                Err(ParseError::ParseError(
                    "custom sort not supported for multiple datasets".to_string(),
                ))
            } else {
                match datatypes.iter().next() {
                    Some(datatype) => Ok(HashMap::from_iter([(*datatype, Some(raw_sort.clone()))])),
                    None => Err(ParseError::ParseError("schemas map is empty".to_string())),
                }
            }
        }
    }
}
