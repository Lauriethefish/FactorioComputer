//! Types/methods for manipulating factorio blueprints.

use std::io::Write;

use base64::Engine;
use deflate::{Compression, write::ZlibEncoder};
use serde::{Serialize, Deserialize};

use crate::assembly::Instruction;

#[derive(Serialize, Deserialize)]
pub struct SerializedBlueprint {
    pub blueprint: Blueprint
}

#[derive(Serialize, Deserialize)]
pub struct Blueprint {
    pub item: String,
    pub label: String,
    pub entities: Vec<Entity>,
    pub version: u32
}

#[derive(Serialize, Deserialize)]
pub struct Entity {
    pub entity_number: u32,
    pub name: String,
    pub position: Position,
    pub direction: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub connections: Option<Connection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub control_behavior: Option<ControlBehaviour>
}

#[derive(Serialize, Deserialize)]
pub struct Connection {
    #[serde(rename = "1")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a: Option<ConnectionPoint>,
    #[serde(rename = "2")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub b: Option<ConnectionPoint>
}

#[derive(Serialize, Deserialize)]
pub struct ControlBehaviour {
    //arithmetic_conditions: Option<ArithmeticCombinatorParameters>,
    pub decider_conditions: Option<DeciderCombinatorParameters>,
    pub filters: Option<Vec<ConstantCombinatorParameter>>
}

#[derive(Serialize, Deserialize)]
pub struct ConnectionPoint {
    pub red: Vec<ConnectionData>,
    pub green: Vec<ConnectionData>
}

#[derive(Serialize, Deserialize)]
pub struct ConnectionData {
    pub entity_id: u32,
    pub circuit_id: u32
}

#[derive(Serialize, Deserialize)]
pub struct Position {
    pub x: f32,
    pub y: f32
}

#[derive(Serialize, Deserialize)]
pub struct DeciderCombinatorParameters {
    pub comparator: char,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub first_signal: Option<SignalId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub second_signal: Option<SignalId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constant: Option<i32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_signal: Option<SignalId>,
    pub copy_count_from_input: bool
}

#[derive(Serialize, Deserialize)]
pub struct ConstantCombinatorParameter {
    pub signal: SignalId,
    pub count: i32,
    pub index: u32
}

#[derive(Serialize, Deserialize, Clone)]
pub struct SignalId {
    pub r#type: String,
    pub name: String
}

impl SerializedBlueprint {
    pub fn save(&self) -> String {
        let bytes = serde_json::to_string_pretty(self)
            .expect("Failed to serialize blueprint");
        
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::Best);
        encoder.write_all(&bytes.as_bytes()).unwrap();
        let compressed_data = encoder.finish().unwrap();
    
        let encoded = base64::engine::general_purpose::STANDARD_NO_PAD.encode(compressed_data);

        return format!("0{encoded}");
    }
}

// Generates a blueprint containing a program ROM with the given instructions.
pub fn generate_rom_blueprint(instructions: &[Instruction]) -> Blueprint {
    let mut entities = Vec::new();

    let program_addr_signal = SignalId {
        r#type: "virtual".to_owned(),
        name: "signal-P".to_owned(),
    };

    let all_signal = SignalId {
        r#type: "virtual".to_owned(),
        name: "signal-everything".to_owned(),
    };

    let opcode_signal = SignalId {
        r#type: "virtual".to_owned(),
        name: "signal-O".to_owned(),
    };

    for (idx, instruction) in instructions.iter().enumerate() {
        entities.push(Entity {
            entity_number: (entities.len() + 1) as u32,
            name: "decider-combinator".to_owned(),
            position: Position { x: 0.0, y: -(idx as f32) },
            direction: 2,
            connections: if entities.len() == 0 {
                None
            } else {
                Some(Connection {
                    b: Some(ConnectionPoint {
                        red: vec![ConnectionData { entity_id: (entities.len() - 1) as u32, circuit_id: 2 }],
                        green: vec![]
                    }),
                    a: Some(ConnectionPoint {
                        red: vec![ConnectionData { entity_id: (entities.len() - 1) as u32, circuit_id: 1 }],
                        green: vec![]
                    }),
                })
            },
            control_behavior: Some(ControlBehaviour {
                decider_conditions: Some(DeciderCombinatorParameters {
                    comparator: '=',
                    first_signal: Some(program_addr_signal.clone()),
                    second_signal: None,
                    constant: Some((idx + 1) as i32), // First instruction is index 1
                    output_signal: Some(all_signal.clone()),
                    copy_count_from_input: true,
                }),
                filters: None,
            })
        });

        let mut filters = vec![
            ConstantCombinatorParameter {
                signal: opcode_signal.clone(),
                count: instruction.get_opcode(),
                index: 1
            }
        ];

        match instruction.get_argument_signal() {
            Some((signal, count)) => filters.push(ConstantCombinatorParameter {
                signal,
                count,
                index: 2
            }),
            None => {}
        };

        entities.push(Entity {
            entity_number: (entities.len() + 1) as u32,
            name: "constant-combinator".to_owned(),
            position: Position { x: -2.0, y: -(idx as f32) },
            direction: 1,
            connections: Some(Connection {
                b: None,
                a: Some(ConnectionPoint {
                    green: vec![ConnectionData { entity_id: (entities.len()) as u32, circuit_id: 1 }],
                    red: vec![]
                }),
            }),
            control_behavior: Some(ControlBehaviour {
                decider_conditions: None,
                filters: Some(filters),
            })
        });
    }

    Blueprint {
        item: "blueprint".to_string(),
        label: "Program".to_string(),
        entities,
        version: 0,
    }
}