use ureq::{Agent, AgentBuilder, Cookie};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use serde::{Deserialize};
use clap::ArgMatches;
use std::error::Error;
use std::io::BufRead;
use std::os::unix::net::UnixStream;
use thrift::protocol::{TBinaryInputProtocol, TBinaryOutputProtocol};

mod tls;
mod passwordless_login;

pub use passwordless_login::*;


#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClusterStatus {
    cluster_status: ClusterStatus_
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ClusterStatus_ {
    nodes: Vec<NodeStatus>,
    rollup_status: String,
    rollup_requested_deployment_state: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeStatus {
    services: Vec<ServiceStatus>,
    node_id: String,
    rollup_status: String,
    rollup_requested_deployment_state: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceStatus {
    service_name: String,
    instances: Vec<InstanceStatus>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InstanceStatus {
    code: Option<String>,
    process_status: String,
    instance_id: String,
    message: Option<String>,
    timestamp_utc: u64,
    current_deployment_state: String,
}

fn get_status_as_value(status: &String, deployment: Option<&String>) -> i8 {


    if status.eq("Active") || status.eq("Enabled") || status.eq("Running") {
        0
    } else if status.eq("Busy") || status.eq("Passive") {
        1
    } else {
        match deployment {
            None => 2,
            Some(state) => {
                if state.eq("Disabled") {
                    -1
                } else {
                    2
                }
            }
        }
    }
}

fn get_epoch_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos()
}

fn get_system_info_xml(agent: &Agent, url: &String) -> Result<(String, u128), ureq::Error> {
    let start = Instant::now();

    let xml_server_info = agent.get(url)
        .call()?
        .into_string()?;

    Ok((xml_server_info, start.elapsed().as_micros()))
}

fn parse_system_info(xml: &String, elapsed: u128) -> Result<(), roxmltree::Error> {
    let doc = roxmltree::Document::parse(&*xml)?;

    for node in doc.descendants() {
        let tag_name = node.tag_name().name();

        if tag_name == "" || tag_name == "systeminfo" || tag_name == "machines" || tag_name == "machine" {
            continue;
        }

        let status = node.attribute("status").unwrap_or("Unknown");

        if tag_name == "service" {
            println!("tableau_systeminfo,worker=all status_code={}i,status=\"{}\",elapsed={}i {}"
                     , get_status_as_value(&status.to_string(),None)
                     , status
                     , elapsed
                     , get_epoch_nanos())
        } else {
            let worker = node.attribute("worker").unwrap_or("Unknown");
            println!("tableau_systeminfo,process={},worker={} status_code={}i,status=\"{}\" {}"
                     , tag_name
                     , worker
                     , get_status_as_value(&status.to_string(),None)
                     , status
                     , get_epoch_nanos());
        }
    };

    Ok(())
}

fn check_system_info(agent: &Agent, url: &str) -> Result<(), Box<dyn Error>> {
    let url = std::format!("{}admin/systeminfo.xml", url);

    match get_system_info_xml(&agent, &url) {
        Ok((xml, elapsed)) => Ok(parse_system_info(&xml, elapsed)?),
        Err(e) => Err(Box::new(e))
    }
}

fn get_passwordless_cookie(name: Option<String>, value: Option<String>) -> String {
    match (name,value) {
        (Some(name),Some(value)) => {
            Cookie::build(name, value)
                .domain("localhost")
                .path("/")
                .secure(true)
                .http_only(true)
                .finish()
                .to_string()
        },
        _ => "".to_string()
    }
}

fn check_tsm_nodes(agent: &Agent, args: &ArgMatches) -> Result<(), Box<dyn Error>> {
    let tsm_host = args.value_of("tsm_hostname").expect("tsm_hostname must be defined");

    let logon_url = std::format!("{}api/0.5/login",tsm_host);
    let status_url = std::format!("{}api/0.5/status", tsm_host);

    let start = Instant::now();

    let status_req;


    if cfg!(unix) && args.is_present("passwordless") {
        let tsm_socket = args.value_of("tsm_socket").expect("TSM socket must be defined");
        let login_result = get_passwordless_result(tsm_socket)?;
        let cookie = get_passwordless_cookie(login_result.cookie_name, login_result.cookie_value);
        status_req = agent.get(&status_url)
            .set("Cookie", &cookie.as_str());
    } else {
        agent.post(&logon_url)
            .send_json(ureq::json!({
            "authentication": {
                "name": args.value_of("tsm_user").expect("TSM username must be defined") ,
                "password": args.value_of("tsm_password").expect("TSM password var must be defined")
            }}))?
            .into_string()?;
        status_req = agent.get(&status_url);
    }


    let status: ClusterStatus = status_req
        .call()?
        .into_json()?;
    let cluster_status = status.cluster_status;

    // Cluster level
    println!("tableau_tsm_status,node=all,service=all,instance=all status_code={}i,status=\"{}\"\
        ,requested_deployment_state=\"{}\",elapsed={}i {}",
             get_status_as_value(&cluster_status.rollup_status,
                                 Some(&cluster_status.rollup_requested_deployment_state)),
             cluster_status.rollup_status,
             cluster_status.rollup_requested_deployment_state,
             start.elapsed().as_micros(),
             get_epoch_nanos()
    );

    // Node Level
    for node in cluster_status.nodes {
        println!("tableau_tsm_status,node={},service=all,instance=all \
            status_code={}i,status=\"{}\",requested_deployment_state=\"{}\" {}",
                 node.node_id,
                 get_status_as_value(&node.rollup_status,
                                     Some(&node.rollup_requested_deployment_state)),
                 node.rollup_status,
                 node.rollup_requested_deployment_state,
                 get_epoch_nanos()
        );

        // Instance Level
        for service in node.services {
            for instance in service.instances {
                println!("tableau_tsm_status,node={},service={},instance={} \
                            status_code={}i,status=\"{}\",deployment_state=\"{}\"\
                            ,message=\"{}\",code=\"{}\",timestamp_utc={}i \
                            {}",
                         node.node_id,
                         service.service_name,
                         instance.instance_id,
                         get_status_as_value(&instance.process_status,
                                             Some(&instance.current_deployment_state)),
                         instance.process_status,
                         instance.current_deployment_state,
                         instance.message.unwrap_or("".to_string()),
                         instance.code.unwrap_or("".to_string()),
                         instance.timestamp_utc,
                         get_epoch_nanos()
                );
            }
        }
    }


    Ok(())
}

#[cfg(unix)]
pub fn get_passwordless_result(socket_path: &str) -> thrift::Result<PasswordLessLoginResult> {
    let socket_tx = UnixStream::connect(socket_path)?;
    let socket_rx = socket_tx.try_clone()?;

    let in_proto = TBinaryInputProtocol::new(socket_tx, true);
    let out_proto = TBinaryOutputProtocol::new(socket_rx, true);
    let mut client = PasswordLessLoginSyncClient::new(in_proto, out_proto);

    let passwordless_login_result = client.login();

    passwordless_login_result
}

pub fn run(args: &ArgMatches) {
    let hostname = args.value_of("systeminfo_hostname").expect("Missing Server hostname");
    let checks = args.value_of("checks").expect("No checks are defined.");

    let mut tls_config = rustls::ClientConfig::new();
    tls_config
        .dangerous()
        .set_certificate_verifier(Arc::new(crate::tls::NoCertificateVerification {}));

    let agent: Agent = AgentBuilder::new()
        .timeout_read(Duration::from_secs(5))
        .timeout_write(Duration::from_secs(5))
        .tls_config(Arc::new(tls_config))
        .build();

    for _ in std::io::stdin().lock().lines() {
        if checks.eq("all") || checks.eq("tsm") {
            if let Err(e) = check_tsm_nodes(&agent, &args) {
                println!("tableau_tsm_status,node=all,service=all,instance=all status_code=3i,\
                status=\"Unavailable\",requested_deployment_state=\"Unknown\" {}",
                         get_epoch_nanos());
                eprintln!("check_tsm_nodes error: {}", e.to_string());
            }
        }

        if checks.eq("all") || checks.eq("systeminfo") {
            if let Err(e) = check_system_info(&agent, hostname) {
                println!("tableau_systeminfo,worker=all status_code=3i,status=\"Unavailable\" {}"
                         , get_epoch_nanos());
                eprintln!("check_system_info error: {}", e.to_string());
            }
        }
    }
}
