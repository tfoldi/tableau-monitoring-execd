use ureq::{Agent, AgentBuilder};
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};
use std::sync::Arc;
use serde::{Deserialize};
use clap::ArgMatches;
use std::error::Error;

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
    rollup_requested_deployment_state: String
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct NodeStatus {
    services: Vec<ServiceStatus>,
    node_id: String,
    rollup_status: String,
    rollup_requested_deployment_state: String
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ServiceStatus {
    service_name: String,
    instances: Vec<InstanceStatus>
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


fn get_epoch_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
}

fn get_system_info_xml(agent: &Agent, url: &String) -> Result<String, ureq::Error> {
    let xml_server_info = agent.get(url)
        .call()?
        .into_string()?;

    Ok(xml_server_info)
}

fn parse_system_info(xml: &String) -> Result<(), roxmltree::Error> {
    let doc = roxmltree::Document::parse(&*xml)?;

    for node in doc.descendants() {
        let tag_name = node.tag_name().name();

        if tag_name == "" || tag_name == "systeminfo" || tag_name == "machines" || tag_name == "machine" {
            continue;
        }

        let status = node.attribute("status").unwrap_or("Unknown");

        if tag_name == "service" {
            println!("tableau_systeminfo,worker=all status=\"{}\" {}", status, get_epoch_ms())
        } else {
            let worker = node.attribute("worker").unwrap_or("Unknown");
            println!("tableau_systeminfo,process={},worker={} status=\"{}\" {}"
                     , tag_name
                     , worker
                     , status
                     , get_epoch_ms());
        }
    };

    Ok(())
}

fn check_system_info(agent: &Agent, url: &str) -> Result<(), Box<dyn Error>> {
    let url = std::format!("{}admin/systeminfo.xml", url);

    match get_system_info_xml(&agent, &url) {
        Ok(xml) => Ok(parse_system_info(&xml)?),
        Err(e) => Err( Box::new(e) )
    }
}


fn check_tsm_nodes(agent: &Agent, args: &ArgMatches ) -> Result<(), ureq::Error> {
    let tsm_host = args.value_of("tsm_hostname").expect("tsm_hostname must be defined");

    let logon_url = std::format!("{}api/0.5/login",tsm_host);
    let status_url = std::format!("{}api/0.5/status", tsm_host);


    agent.post(&logon_url)
        .send_json(ureq::json!({
            "authentication": {
                "name": args.value_of("tsm_user").expect("TSM username must be defined") ,
                "password": args.value_of("tsm_password").expect("TSM password var must be defined")
            }}))?
        .into_string()?;


    let status : ClusterStatus = agent.get(&status_url)
        .call()?
        .into_json()?;
    let cluster_status = status.cluster_status;

    // Cluster level
    println!("tableau_tsm_status,node=all,service=all,instance=all status=\"{}\",requested_deployment_state=\"{}\" {}",
             cluster_status.rollup_status,
             cluster_status.rollup_requested_deployment_state,
             get_epoch_ms()
    );

    // Node Level
    for node in cluster_status.nodes {
        println!("tableau_tsm_status,node={},service=all,instance=all status=\"{}\",requested_deployment_state={} {}",
                 node.node_id,
                 node.rollup_status,
                 node.rollup_requested_deployment_state,
                 get_epoch_ms()
        );

        // Instance Level
        for service in node.services {
            for instance in service.instances {
                println!("tableau_tsm_status,node={},service={},instance={} \
                            status=\"{}\",deployment_state=\"{}\",message=\"{}\",code=\"{}\",timestamp_utc={}i \
                            {}",
                         node.node_id,
                         service.service_name,
                         instance.instance_id,
                         instance.process_status,
                         instance.current_deployment_state,
                         instance.message.unwrap_or("".to_string()),
                         instance.code.unwrap_or("".to_string()),
                         instance.timestamp_utc,
                         get_epoch_ms()
                );
            }
        }

    }


    Ok(())
}

pub fn run(args: &ArgMatches ) {
    let hostname = args.value_of("systeminfo_hostname").expect("Missing Server hostname");

    let mut tls_config = rustls::ClientConfig::new();
    tls_config
        .dangerous()
        .set_certificate_verifier(Arc::new(crate::tls::NoCertificateVerification {}));

    let agent: Agent = AgentBuilder::new()
        .timeout_read(Duration::from_secs(5))
        .timeout_write(Duration::from_secs(5))
        .tls_config(Arc::new(tls_config))
        .build();

    if let Err(e) = check_tsm_nodes(&agent, &args ) {
        eprintln!("check_tsm_nodes error: {}", e.to_string());
    }

    if let Err(e) = check_system_info(&agent, hostname  ) {
        eprintln!("check_system_info error: {}", e.to_string());
    }
}
