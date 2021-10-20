#[macro_use]
extern crate rocket;
extern crate base64;
use jsonwebtokens as jwt;
use jwt::{Algorithm, AlgorithmID, Verifier};
use lazy_static::lazy_static;
use rocket::form::Form;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use serde::Deserialize;
use serde_json::value::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Mutex;

static SUCCESS_MESSAGE: &str = "Hello, admin!";
static FAIL_MESSAGE: &str = "Who are you? I didn't call you!";

lazy_static! {
    static ref IP_BLACKLIST: Mutex<Vec<IpAddr>> = Mutex::new(vec![]);
    static ref IP_BAD_TRIES: Mutex<HashMap<IpAddr, i32>> = Mutex::new(HashMap::new());
}

#[derive(Debug)]
enum ApiTokenError {
    Missing,
    Invalid,
}

#[derive(FromForm)]
struct Credentials<'r> {
    username: &'r str,
    password: &'r str,
}

#[derive(Debug)]
struct Token(String);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for Token {
    type Error = ApiTokenError;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let token = request.headers().get_one("Authorization");

        fn is_valid(token: &str) -> bool {
            token.starts_with("Bearer ")
        }

        match token {
            // token does not exist
            None => Outcome::Failure((Status::Unauthorized, ApiTokenError::Missing)),
            // token is valid
            Some(token) if is_valid(token) => Outcome::Success(Token(token.to_string())),
            // token is invalid
            Some(_) => Outcome::Failure((Status::Unauthorized, ApiTokenError::Invalid)),
        }
    }
}

#[derive(Debug)]
struct ClientIP(IpAddr);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ClientIP {
    type Error = ApiTokenError;

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let socket = request.remote().unwrap();

        return Outcome::Success(ClientIP(socket.ip()));
    }
}

#[derive(Deserialize)]
struct Username {
    username: String,
}

// Who will guess that username is 'username' and password is 'password'?
#[post("/auth1", data = "<creds>")]
fn auth1(creds: Form<Credentials<'_>>) -> String {
    let mut credentials = HashMap::new();
    credentials.insert(
        "username".to_string(),
        "5e884898da28047151d0e56f8dc6292773603d0d6aabbdd62a11ef721d1542d8".to_string(),
    );

    fn hash_password(password: &str) -> String {
        let mut sha256 = Sha256::new();
        sha256.update(password);
        return format!("{:X}", sha256.finalize()).to_lowercase();
    }

    match credentials.get(creds.username) {
        None => FAIL_MESSAGE.to_string(),
        Some(password) if password.to_string() == hash_password(creds.password) => {
            SUCCESS_MESSAGE.to_string()
        }
        Some(_) => FAIL_MESSAGE.to_string(),
    }
}

// Uning a token is always a good idea! Who knows what's inside of it?)
#[post("/auth2")]
fn auth2(token: Token) -> String {
    let token_parts: Vec<&str> = token.0.split(" ").collect();
    let raw_token = token_parts[1];

    let raw_token_data = base64::decode(raw_token);

    match raw_token_data {
        Ok(_) => (),
        Err(_) => return FAIL_MESSAGE.to_string(),
    };

    let token_data: String = String::from_utf8(raw_token_data.unwrap()).unwrap();

    return match token_data.as_str() {
        "admin" => SUCCESS_MESSAGE.to_string(),
        "user" => SUCCESS_MESSAGE.to_string(),
        _ => FAIL_MESSAGE.to_string(),
    };
}

// We will store hashes in order to protect our user's data, and those hashes will be generated by the front-end, so we will increase our backend performance! What could go wrong?
#[post("/auth3", data = "<creds>")]
fn auth3(creds: Form<Credentials<'_>>) -> String {
    let mut credentials = HashMap::new();
    credentials.insert(
        "admin".to_string(),
        "5e884898da28047151d0e56f8dc6292773603d0d6aabbdd62a11ef721d1542d8".to_string(),
    );

    match credentials.get(creds.username) {
        None => FAIL_MESSAGE.to_string(),
        Some(password) if password.to_string() == creds.password => SUCCESS_MESSAGE.to_string(),
        Some(_) => FAIL_MESSAGE.to_string(),
    }
}

// Now no one will guess the credentials
#[post("/auth1_fix", data = "<creds>")]
fn auth1_fix(creds: Form<Credentials<'_>>, client_ip: ClientIP) -> Result<String, Status> {
    let ip = client_ip.0;

    if IP_BLACKLIST
        .lock()
        .unwrap()
        .iter()
        .any(|black_ip| black_ip.eq(&ip))
    {
        return Err(Status::Locked);
    }

    let mut credentials = HashMap::new();
    credentials.insert(
        "username".to_string(),
        "5e884898da28047151d0e56f8dc6292773603d0d6aabbdd62a11ef721d1542d8".to_string(),
    );

    fn hash_password(password: &str) -> String {
        let mut sha256 = Sha256::new();
        sha256.update(password);
        return format!("{:X}", sha256.finalize()).to_lowercase();
    }

    match credentials.get(creds.username) {
        None => Ok(FAIL_MESSAGE.to_string()),
        Some(password) if password.to_string() == hash_password(creds.password) => {
            Ok(SUCCESS_MESSAGE.to_string())
        }
        Some(_) => {
            let mut ip_bad_tries_entity = IP_BAD_TRIES.lock().unwrap();
            let ibt_counter = ip_bad_tries_entity.entry(ip).or_insert(0);
            *ibt_counter += 1;

            if *ibt_counter >= 3 {
                IP_BLACKLIST.lock().unwrap().push(ip);
            }

            return Ok(FAIL_MESSAGE.to_string());
        }
    }
}

// Uning a token is always a good idea, if you use signs of course.
#[post("/auth2_fix")]
fn auth2_fix(str_token: Token) -> String {
    let alg = Algorithm::new_hmac(AlgorithmID::HS256, "superStrongSecretForTokenSign").unwrap();

    let verifier = Verifier::create()
        .leeway(5) // give this much leeway (in seconds) when validating exp, nbf and iat claims
        .build()
        .unwrap();

    let auth_token: Vec<&str> = str_token.0.split(" ").collect();
    let token: String = auth_token[1].to_string();
    let _claims: Value = verifier.verify(&token, &alg).unwrap();

    let token_parts: Vec<&str> = token.split(".").collect();
    let token_payload = base64::decode(token_parts[1]);

    match token_payload {
        Ok(_) => (),
        Err(_) => return FAIL_MESSAGE.to_string(),
    };

    let username_json: String = String::from_utf8(token_payload.unwrap()).unwrap();
    let username_field: Username = serde_json::from_str(&username_json).unwrap();
    let username = username_field.username;

    return match username.as_str() {
        "admin" => SUCCESS_MESSAGE.to_string(),
        "user" => SUCCESS_MESSAGE.to_string(),
        _ => FAIL_MESSAGE.to_string(),
    };
}

// We will do everything in accordance to best practice, and will not send hashes through internet.
#[post("/auth3_fix", data = "<creds>")]
fn auth3_fix(creds: Form<Credentials<'_>>) -> String {
    let mut credentials = HashMap::new();
    credentials.insert(
        "admin".to_string(),
        "5e884898da28047151d0e56f8dc6292773603d0d6aabbdd62a11ef721d1542d8".to_string(),
    );

    fn hash_password(password: &str) -> String {
        let mut sha256 = Sha256::new();
        sha256.update(password);
        return format!("{:X}", sha256.finalize()).to_lowercase();
    }

    match credentials.get(creds.username) {
        None => FAIL_MESSAGE.to_string(),
        Some(password) if password.to_string() == hash_password(creds.password) => {
            SUCCESS_MESSAGE.to_string()
        }
        Some(_) => FAIL_MESSAGE.to_string(),
    }
}

#[launch]
fn rocket() -> _ {
    rocket::build().mount(
        "/",
        routes![auth1, auth2, auth3, auth1_fix, auth2_fix, auth3_fix],
    )
}
