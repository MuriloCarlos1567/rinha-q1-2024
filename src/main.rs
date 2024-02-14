use std::{collections::HashMap, sync::Arc};

use axum::{
    body::Body,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

#[derive(Clone, Serialize, Deserialize, Debug)]
pub struct Statement {
    pub id: i32,
    pub valor: i32,
    pub tipo: String,
    pub descricao: String,
    pub realizado_em: String,
    pub user_id: i32,
}

#[derive(Serialize, Deserialize)]
pub struct LastTransaction {
    pub valor: i32,
    pub tipo: String,
    pub descricao: String,
    pub realizado_em: String,
}

#[derive(Serialize, Deserialize)]
pub struct Balance {
    pub total: i32,
    pub data_extrato: String,
    pub limite: i32,
}

#[derive(Serialize, Deserialize)]
pub struct StatementResponse {
    pub saldo: Balance,
    pub ultimas_transacoes: Vec<LastTransaction>,
}

#[derive(Serialize, Deserialize)]
pub struct TransactionResponse {
    pub limite: i32,
    pub saldo: i32,
}

#[derive(Serialize, Deserialize)]
pub struct NewTransaction {
    pub valor: i32,
    pub tipo: String,
    pub descricao: String,
}

#[derive(Clone)]
pub struct User {
    pub id: i32,
    pub limite: i32,
    pub saldo: i32,
}

enum StatementResult {
    Success(Json<StatementResponse>),
    NotFound,
}

impl IntoResponse for StatementResult {
    fn into_response(self) -> axum::response::Response {
        match self {
            StatementResult::Success(json) => json.into_response(),
            StatementResult::NotFound => StatusCode::NOT_FOUND.into_response(),
        }
    }
}

enum TransactionResult {
    Success(Json<TransactionResponse>),
    NotFound,
    UnprocessableEntity,
}

impl IntoResponse for TransactionResult {
    fn into_response(self) -> axum::response::Response<Body> {
        match self {
            TransactionResult::Success(json) => json.into_response(),
            TransactionResult::NotFound => StatusCode::NOT_FOUND.into_response(),
            TransactionResult::UnprocessableEntity => {
                StatusCode::UNPROCESSABLE_ENTITY.into_response()
            }
        }
    }
}

type ArcState = Arc<Mutex<HashMap<i32, User>>>;
type StatementState = Arc<Mutex<HashMap<i32, Statement>>>;

#[derive(Clone)]
pub struct AppState {
    user_state: ArcState,
    statement_state: StatementState,
}

impl AppState {
    fn new() -> Self {
        let mut hash_user: HashMap<i32, User> = HashMap::new();
        let hash_statement: HashMap<i32, Statement> = HashMap::new();

        hash_user.insert(
            1,
            User {
                id: 1,
                limite: 100000,
                saldo: 0,
            },
        );
        hash_user.insert(
            2,
            User {
                id: 2,
                limite: 80000,
                saldo: 0,
            },
        );
        hash_user.insert(
            3,
            User {
                id: 3,
                limite: 1000000,
                saldo: 0,
            },
        );
        hash_user.insert(
            4,
            User {
                id: 4,
                limite: 10000000,
                saldo: 0,
            },
        );
        hash_user.insert(
            5,
            User {
                id: 5,
                limite: 500000,
                saldo: 0,
            },
        );

        AppState {
            user_state: Arc::new(Mutex::new(hash_user)),
            statement_state: Arc::new(Mutex::new(hash_statement)),
        }
    }
}

#[tokio::main]
async fn main() {
    let app_state: AppState = AppState::new();

    let app = Router::new()
        .route("/clientes/:id/transacoes", post(create_transaction))
        .route("/clientes/:id/extrato", get(get_bank_statement))
        .with_state(app_state.clone());

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

async fn get_bank_statement(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
) -> impl IntoResponse {
    let users = state.user_state.lock().await;
    let statements = state.statement_state.lock().await;

    if let Some(user) = users.get(&user_id) {
        let balance = Balance {
            total: user.saldo,
            data_extrato: Utc::now().to_rfc3339(),
            limite: user.limite,
        };

        let mut last_transactions: Vec<LastTransaction> = Vec::new();

        for (_, statement) in statements.iter().filter(|(_, s)| s.user_id == user_id) {
            if last_transactions.len() >= 10 {
                break;
            }

            last_transactions.push(LastTransaction {
                valor: statement.valor,
                tipo: statement.tipo.to_string(),
                descricao: statement.descricao.clone(),
                realizado_em: statement.realizado_em.clone(),
            })
        }

        StatementResult::Success(Json(StatementResponse {
            saldo: balance,
            ultimas_transacoes: last_transactions,
        }))
    } else {
        StatementResult::NotFound
    }
}

async fn create_transaction(
    State(state): State<AppState>,
    Path(user_id): Path<i32>,
    Json(new_statement): Json<NewTransaction>,
) -> impl IntoResponse {
    let mut users = state.user_state.lock().await;
    let mut statements = state.statement_state.lock().await;

    if let Some(user) = users.get_mut(&user_id) {
        let new_balance = match new_statement.tipo.as_str() {
            "c" => {
                let balance = user.saldo + new_statement.valor;
                user.saldo = balance;

                let hack_id: i32 = (statements.len() + 1) as i32;

                statements.insert(
                    hack_id,
                    Statement {
                        id: hack_id,
                        valor: new_statement.valor,
                        tipo: new_statement.tipo,
                        descricao: new_statement.descricao,
                        realizado_em: Utc::now().to_rfc3339(),
                        user_id: user_id,
                    },
                );

                TransactionResult::Success(Json(TransactionResponse {
                    limite: user.limite,
                    saldo: balance,
                }))
            }
            "d" => {
                let new_balance = user.saldo - new_statement.valor;
                if new_balance < -user.limite {
                    return TransactionResult::UnprocessableEntity;
                } else {
                    user.saldo = new_balance;
                    TransactionResult::Success(Json(TransactionResponse {
                        limite: user.limite,
                        saldo: new_balance,
                    }))
                }
            }
            _ => TransactionResult::UnprocessableEntity,
        };
        new_balance
    } else {
        TransactionResult::NotFound
    }
}
