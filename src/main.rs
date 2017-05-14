// This Source Code Form is subject to the terms of the Mozilla Public
// License, v. 2.0. If a copy of the MPL was not distributed with this
// file, You can obtain one at http://mozilla.org/MPL/2.0/.

/// Simple server that manages foxbox registrations.
/// Two end points are available:
/// POST /register => to register a match between public IP and mesage.
/// GET /ping => to get the list of public IP matches.
///
/// Boxes are supposed to register themselves at regular intervals so we
/// discard data which is too old periodically.
#[macro_use]
extern crate clap;
extern crate crypto;
extern crate env_logger;
extern crate hyper_openssl;
#[macro_use]
extern crate iron;
extern crate iron_cors;
#[macro_use]
extern crate log;
extern crate mount;
extern crate params;
extern crate r2d2;
extern crate r2d2_sqlite;
extern crate router;
extern crate rusqlite;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate serde_json;
extern crate toml;
extern crate uuid;

use hyper_openssl::OpensslServer;
use iron::{Chain, Iron};
use iron::method::Method;
use iron_cors::CORS;
use mount::Mount;

macro_rules! json_response {
    ($json:expr) => (
        {
            let mut response = Response::with(serde_json::to_string($json).unwrap());
            response.headers.set(ContentType::json());
            Ok(response)
        }
    )
}

macro_rules! ok_response {
    () => (
        {
            let mut response = Response::new();
            response.status = Some(Status::Ok);
            Ok(response)
        }
    )
}

mod args;
mod config;
mod database;
mod discovery;
mod errors;
mod eviction;
mod pdns;
mod routes;

use args::Args;

fn main() {
    env_logger::init().unwrap();

    let args = Args::new();

    info!("Managing the domain {}", args.domain);

    let config = args.to_config();

    eviction::evict_old_entries(&config);

    let mut mount = Mount::new();
    mount.mount("/", routes::create(&config));

    let mut chain = Chain::new(mount);
    let cors = CORS::new(vec![(vec![Method::Get], "info".to_owned()),
                              (vec![Method::Get], "subscribe".to_owned()),
                              (vec![Method::Get], "unsubscribe".to_owned()),
                              (vec![Method::Get], "register".to_owned()),
                              (vec![Method::Get], "dnsconfig".to_owned()),
                              (vec![Method::Get], "ping".to_owned()),
                              (vec![Method::Get], "adddiscovery".to_owned()),
                              (vec![Method::Get], "revokediscovery".to_owned()),
                              (vec![Method::Get], "discovery".to_owned())]);
    chain.link_after(cors);

    let iron = Iron::new(chain);
    info!("Starting server on {}:{}", args.host, args.port);
    let addr = format!("{}:{}", args.host, args.port);

    pdns::start_socket_endpoint(&config);

    if args.cert_directory.is_none() {
        iron.http(addr.as_ref() as &str).unwrap();
    } else {
        info!("Starting TLS server");
        let certificate_directory = args.cert_directory.unwrap();

        let mut private_key = certificate_directory.clone();
        private_key.push("privkey.pem");

        let mut cert = certificate_directory.clone();
        cert.push("fullchain.pem");

        info!("Using cert: '{:?}' pk: '{:?}'", cert, private_key);
        let ssl = OpensslServer::from_files(private_key, cert).unwrap();
        iron.https(addr.as_ref() as &str, ssl).unwrap();
    }
}

// TODO: add iron tests.
