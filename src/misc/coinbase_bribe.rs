use alloy::rpc::types::trace::{
    geth::CallFrame,
    parity::{Action, TransactionTrace},
};
use revm::primitives::{Address, U256};

pub struct TraceData {
    pub to: Option<Address>,
    pub value: Option<U256>,
}

pub fn find_coinbase_transfer(coinbase: Address, traces: Vec<TraceData>) -> U256 {
    for trace in traces {
        if let Some(to) = trace.to {
            if to == coinbase {
                if let Some(value) = trace.value {
                    return value;
                }
            }
        }
    }

    U256::ZERO
}

impl From<TransactionTrace> for TraceData {
    fn from(trace: TransactionTrace) -> Self {
        match trace.action {
            Action::Call(call_data) => TraceData {
                to: Some(call_data.to),
                value: Some(call_data.value),
            },
            _ => TraceData {
                to: None,
                value: None,
            },
        }
    }
}

impl From<CallFrame> for TraceData {
    fn from(call: CallFrame) -> Self {
        TraceData {
            to: call.to,
            value: call.value,
        }
    }
}
