use cosmwasm_schema::{cw_serde, QueryResponses};
use cosmwasm_std::{
    entry_point, to_json_binary, Binary, Deps, DepsMut, Env, MessageInfo, Response, StdResult,
};
use cw_storage_plus::Item;

// ============================================================================
// STATE
// ============================================================================

const COUNTER: Item<i64> = Item::new("counter");
const OWNER: Item<String> = Item::new("owner");

// ============================================================================
// MESSAGES
// ============================================================================

#[cw_serde]
pub struct InstantiateMsg {
    pub initial_count: i64,
}

#[cw_serde]
pub enum ExecuteMsg {
    Increment {},
    Decrement {},
    Reset { count: i64 },
}

#[cw_serde]
#[derive(QueryResponses)]
pub enum QueryMsg {
    #[returns(CountResponse)]
    GetCount {},
}

#[cw_serde]
pub struct CountResponse {
    pub count: i64,
}

// ============================================================================
// ENTRY POINTS
// ============================================================================

#[entry_point]
pub fn instantiate(
    deps: DepsMut,
    _env: Env,
    info: MessageInfo,
    msg: InstantiateMsg,
) -> StdResult<Response> {
    COUNTER.save(deps.storage, &msg.initial_count)?;
    OWNER.save(deps.storage, &info.sender.to_string())?;
    Ok(Response::new()
        .add_attribute("method", "instantiate")
        .add_attribute("count", msg.initial_count.to_string()))
}

#[entry_point]
pub fn execute(
    deps: DepsMut,
    _env: Env,
    _info: MessageInfo,
    msg: ExecuteMsg,
) -> StdResult<Response> {
    match msg {
        ExecuteMsg::Increment {} => {
            COUNTER.update(deps.storage, |count| -> StdResult<i64> { Ok(count + 1) })?;
            Ok(Response::new().add_attribute("method", "increment"))
        }
        ExecuteMsg::Decrement {} => {
            COUNTER.update(deps.storage, |count| -> StdResult<i64> { Ok(count - 1) })?;
            Ok(Response::new().add_attribute("method", "decrement"))
        }
        ExecuteMsg::Reset { count } => {
            COUNTER.save(deps.storage, &count)?;
            Ok(Response::new().add_attribute("method", "reset"))
        }
    }
}

#[entry_point]
pub fn query(deps: Deps, _env: Env, msg: QueryMsg) -> StdResult<Binary> {
    match msg {
        QueryMsg::GetCount {} => {
            let count = COUNTER.load(deps.storage)?;
            to_json_binary(&CountResponse { count })
        }
    }
}
