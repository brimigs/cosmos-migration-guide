use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdError,
    StdResult, Uint128, WasmMsg,
};
use cw20::Cw20ExecuteMsg;
use cw_storage_plus::{Item, Map};
use thiserror::Error;

const ESCROW: Item<EscrowConfig> = Item::new("escrow");
const ALLOWED_TOKENS: Map<&str, bool> = Map::new("allowed_tokens");
const RECEIPTS: Map<(&str, &str), Receipt> = Map::new("receipts");

#[cw_serde]
pub struct EscrowConfig {
    pub admin: String,
    pub escrow_seed: String,
}

#[cw_serde]
pub struct Receipt {
    pub depositor: String,
    pub token_address: String,
    pub receipt_seed: String,
    pub amount: Uint128,
    pub deposited_at: u64,
}

#[cw_serde]
pub struct InstantiateMsg {
    pub admin: String,
    pub escrow_seed: String,
}

#[cw_serde]
pub enum ExecuteMsg {
    AllowToken { token_address: String },
    Deposit {
        token_address: String,
        receipt_seed: String,
        amount: Uint128,
    },
    Withdraw {
        token_address: String,
        receipt_seed: String,
    },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(EscrowResponse)]
    GetEscrow {},
    #[returns(AllowedTokenResponse)]
    IsTokenAllowed { token_address: String },
    #[returns(ReceiptResponse)]
    GetReceipt { depositor: String, receipt_seed: String },
}

#[cw_serde]
pub struct EscrowResponse {
    pub admin: String,
    pub escrow_seed: String,
}

#[cw_serde]
pub struct AllowedTokenResponse {
    pub token_address: String,
    pub allowed: bool,
}

#[cw_serde]
pub struct ReceiptResponse {
    pub depositor: String,
    pub token_address: String,
    pub receipt_seed: String,
    pub amount: Uint128,
    pub deposited_at: u64,
}

#[derive(Error, Debug)]
pub enum ContractError {
    #[error("{0}")]
    Std(#[from] StdError),

    #[error("unauthorized")]
    Unauthorized,

    #[error("token not allowed")]
    TokenNotAllowed,

    #[error("zero deposit amount")]
    ZeroDepositAmount,

    #[error("receipt already exists")]
    ReceiptAlreadyExists,

    #[error("receipt not found")]
    ReceiptNotFound,
}

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    let admin = deps.api.addr_validate(&msg.admin)?;

    ESCROW.save(
        deps.storage,
        &EscrowConfig {
            admin: admin.to_string(),
            escrow_seed: msg.escrow_seed.clone(),
        },
    )?;

    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("admin", admin)
        .add_attribute("escrow_seed", msg.escrow_seed))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    env: Env,
    info: MessageInfo,
    msg: ExecuteMsg,
) -> Result<Response, ContractError> {
    match msg {
        ExecuteMsg::AllowToken { token_address } => {
            let escrow = ESCROW.load(deps.storage)?;
            let token_addr = deps.api.addr_validate(&token_address)?;
            if info.sender != deps.api.addr_validate(&escrow.admin)? {
                return Err(ContractError::Unauthorized);
            }

            ALLOWED_TOKENS.save(deps.storage, token_addr.as_str(), &true)?;

            Ok(Response::new()
                .add_attribute("method", "allow_token")
                .add_attribute("token_address", token_addr))
        }
        ExecuteMsg::Deposit {
            token_address,
            receipt_seed,
            amount,
        } => {
            if amount.is_zero() {
                return Err(ContractError::ZeroDepositAmount);
            }
            let token_addr = deps.api.addr_validate(&token_address)?;
            let allowed = ALLOWED_TOKENS
                .may_load(deps.storage, token_addr.as_str())?
                .unwrap_or(false);
            if !allowed {
                return Err(ContractError::TokenNotAllowed);
            }

            let depositor = info.sender.to_string();
            let key = (depositor.as_str(), receipt_seed.as_str());
            if RECEIPTS.may_load(deps.storage, key)?.is_some() {
                return Err(ContractError::ReceiptAlreadyExists);
            }

            let receipt = Receipt {
                depositor: depositor.clone(),
                token_address: token_addr.to_string(),
                receipt_seed: receipt_seed.clone(),
                amount,
                deposited_at: env.block.time.seconds(),
            };
            RECEIPTS.save(deps.storage, key, &receipt)?;

            Ok(Response::new()
                .add_message(WasmMsg::Execute {
                    contract_addr: token_addr.to_string(),
                    msg: to_json_binary(&Cw20ExecuteMsg::TransferFrom {
                        owner: depositor.clone(),
                        recipient: env.contract.address.to_string(),
                        amount,
                    })?,
                    funds: vec![],
                })
                .add_attribute("method", "deposit")
                .add_attribute("depositor", depositor)
                .add_attribute("token_address", token_addr)
                .add_attribute("receipt_seed", receipt_seed)
                .add_attribute("amount", amount.to_string()))
        }
        ExecuteMsg::Withdraw {
            token_address,
            receipt_seed,
        } => {
            let token_addr = deps.api.addr_validate(&token_address)?;
            let depositor = info.sender.to_string();
            let key = (depositor.as_str(), receipt_seed.as_str());
            let receipt = RECEIPTS
                .may_load(deps.storage, key)?
                .ok_or(ContractError::ReceiptNotFound)?;

            if receipt.token_address != token_addr.as_str() {
                return Err(ContractError::ReceiptNotFound);
            }

            RECEIPTS.remove(deps.storage, key);

            Ok(Response::new()
                .add_message(WasmMsg::Execute {
                    contract_addr: token_addr.to_string(),
                    msg: to_json_binary(&Cw20ExecuteMsg::Transfer {
                        recipient: depositor.clone(),
                        amount: receipt.amount,
                    })?,
                    funds: vec![],
                })
                .add_attribute("method", "withdraw")
                .add_attribute("depositor", depositor)
                .add_attribute("token_address", token_addr)
                .add_attribute("receipt_seed", receipt_seed)
                .add_attribute("amount", receipt.amount.to_string()))
        }
    }
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetEscrow {} => {
            let escrow = ESCROW.load(deps.storage)?;
            to_json_binary(&EscrowResponse {
                admin: escrow.admin,
                escrow_seed: escrow.escrow_seed,
            })
        }
        QueryMsg::IsTokenAllowed { token_address } => {
            let token_addr = deps.api.addr_validate(&token_address)?;
            let allowed = ALLOWED_TOKENS
                .may_load(deps.storage, token_addr.as_str())?
                .unwrap_or(false);
            to_json_binary(&AllowedTokenResponse {
                token_address: token_addr.to_string(),
                allowed,
            })
        }
        QueryMsg::GetReceipt {
            depositor,
            receipt_seed,
        } => {
            let depositor_addr = deps.api.addr_validate(&depositor)?;
            let receipt = RECEIPTS
                .may_load(deps.storage, (depositor_addr.as_str(), receipt_seed.as_str()))?
                .ok_or_else(|| StdError::not_found("receipt"))?;
            to_json_binary(&ReceiptResponse {
                depositor: receipt.depositor,
                token_address: receipt.token_address,
                receipt_seed: receipt.receipt_seed,
                amount: receipt.amount,
                deposited_at: receipt.deposited_at,
            })
        }
    }
}
