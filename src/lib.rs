use ureq::{Agent, AgentBuilder};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use serde::{Deserialize};
use clap::ArgMatches;
use std::error::Error;
use std::io::BufRead;

mod tls;

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

fn get_status_as_value(status: &String) -> u8 {
    if status.eq("Active") || status.eq("Enabled") || status.eq("Running") {
        0
    } else if status.eq("Busy") || status.eq("Passive") {
        1
    } else {
        2
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
                     , get_status_as_value(&status.to_string())
                     , status
                     , elapsed
                     , get_epoch_nanos())
        } else {
            let worker = node.attribute("worker").unwrap_or("Unknown");
            println!("tableau_systeminfo,process={},worker={} status_code={}i,status=\"{}\" {}"
                     , tag_name
                     , worker
                     , get_status_as_value(&status.to_string())
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


fn check_tsm_nodes(agent: &Agent, args: &ArgMatches) -> Result<(), ureq::Error> {
    let tsm_host = args.value_of("tsm_hostname").expect("tsm_hostname must be defined");

    let logon_url = std::format!("{}api/0.5/login",tsm_host);
    let status_url = std::format!("{}api/0.5/status", tsm_host);

    let start = Instant::now();

    agent.post(&logon_url)
        .send_json(ureq::json!({
            "authentication": {
                "name": args.value_of("tsm_user").expect("TSM username must be defined") ,
                "password": args.value_of("tsm_password").expect("TSM password var must be defined")
            }}))?
        .into_string()?;


    let status: ClusterStatus = agent.get(&status_url)
        .call()?
        .into_json()?;
    let cluster_status = status.cluster_status;

    // Cluster level
    println!("tableau_tsm_status,node=all,service=all,instance=all status_code={}i,status=\"{}\"\
        ,requested_deployment_state=\"{}\",elapsed={}i {}",
             get_status_as_value(&cluster_status.rollup_status),
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
                 get_status_as_value(&node.rollup_status),
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
                         get_status_as_value(&instance.process_status),
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
