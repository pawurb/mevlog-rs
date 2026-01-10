use ratatui::style::Color;
use revm::bytecode::OpCode;

pub trait OpcodeColor {
    fn color(&self) -> Color;
}

impl OpcodeColor for OpCode {
    fn color(&self) -> Color {
        match *self {
            OpCode::DUP1
            | OpCode::DUP2
            | OpCode::DUP3
            | OpCode::DUP4
            | OpCode::DUP5
            | OpCode::DUP6
            | OpCode::DUP7
            | OpCode::DUP8
            | OpCode::DUP9
            | OpCode::DUP10
            | OpCode::DUP11
            | OpCode::DUP12
            | OpCode::DUP13
            | OpCode::DUP14
            | OpCode::DUP15
            | OpCode::DUP16
            | OpCode::SWAP1
            | OpCode::SWAP2
            | OpCode::SWAP3
            | OpCode::SWAP4
            | OpCode::SWAP5
            | OpCode::SWAP6
            | OpCode::SWAP7
            | OpCode::SWAP8
            | OpCode::SWAP9
            | OpCode::SWAP10
            | OpCode::SWAP11
            | OpCode::SWAP12
            | OpCode::SWAP13
            | OpCode::SWAP14
            | OpCode::SWAP15
            | OpCode::SWAP16 => Color::Blue,

            OpCode::PUSH0
            | OpCode::PUSH1
            | OpCode::PUSH2
            | OpCode::PUSH3
            | OpCode::PUSH4
            | OpCode::PUSH5
            | OpCode::PUSH6
            | OpCode::PUSH7
            | OpCode::PUSH8
            | OpCode::PUSH9
            | OpCode::PUSH10
            | OpCode::PUSH11
            | OpCode::PUSH12
            | OpCode::PUSH13
            | OpCode::PUSH14
            | OpCode::PUSH15
            | OpCode::PUSH16
            | OpCode::PUSH17
            | OpCode::PUSH18
            | OpCode::PUSH19
            | OpCode::PUSH20
            | OpCode::PUSH21
            | OpCode::PUSH22
            | OpCode::PUSH23
            | OpCode::PUSH24
            | OpCode::PUSH25
            | OpCode::PUSH26
            | OpCode::PUSH27
            | OpCode::PUSH28
            | OpCode::PUSH29
            | OpCode::PUSH30
            | OpCode::PUSH31
            | OpCode::PUSH32 => Color::Magenta,

            OpCode::LOG0 | OpCode::LOG1 | OpCode::LOG2 | OpCode::LOG3 | OpCode::LOG4 => {
                Color::Yellow
            }

            OpCode::CALL
            | OpCode::STATICCALL
            | OpCode::DELEGATECALL
            | OpCode::CALLCODE
            | OpCode::CREATE
            | OpCode::CREATE2 => Color::Red,

            OpCode::SLOAD | OpCode::SSTORE | OpCode::TLOAD | OpCode::TSTORE => Color::Cyan,

            OpCode::MLOAD | OpCode::MSTORE | OpCode::MSTORE8 | OpCode::MCOPY => Color::Green,

            OpCode::JUMP | OpCode::JUMPI | OpCode::JUMPDEST => Color::LightRed,

            OpCode::REVERT | OpCode::INVALID | OpCode::SELFDESTRUCT => Color::Red,

            OpCode::RETURN | OpCode::STOP => Color::Green,

            _ => Color::White,
        }
    }
}
