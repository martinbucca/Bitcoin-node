use std::{
    net::{Ipv4Addr, SocketAddr, ToSocketAddrs},
    sync::Arc,
};

use crate::{
    config::Config,
    custom_errors::NodeCustomErrors,
    logwriter::log_writer::{write_in_log, LogSender},
};

/// Devuelve una lista de direcciones Ipv4 obtenidas de la DNS seed y de los nodos ingresados manualmente en el archivo de configuración
pub fn get_active_nodes_from_dns_seed(
    config: &Arc<Config>,
    log_sender: &LogSender,
) -> Result<Vec<Ipv4Addr>, NodeCustomErrors> {
    let mut node_ips = Vec::new();
    if config.connect_to_dns_nodes {
        // si en el archivo de configuracion esta seteado que se conecte a los nodos de la dns seed
        get_nodes_from_dns_seed(config, log_sender, &mut node_ips)?;
    }
    for custom_node in config.custom_nodes_ips.iter() {
        // por cada nodo ingresado manualmente en el archivo de configuracion
        let custom_node_ip = match custom_node.parse::<Ipv4Addr>() {
            Ok(ip) => ip,
            Err(err) => {
                write_in_log(
                    &log_sender.error_log_sender,
                    format!(
                        "Error al parsear la ip {} del nodo ingresado manualmente: {}. Debe ser del tipo Ipv4: xxx.x.x.x",
                        custom_node,
                        err
                    )
                    .as_str(),
                );
                continue;
            }
        };
        node_ips.push(custom_node_ip);
    }
    Ok(node_ips)
}

/// Obtiene las direcciones de los nodos a partir de la DNS seed
fn get_nodes_from_dns_seed(
    config: &Arc<Config>,
    log_sender: &LogSender,
    node_ips: &mut Vec<Ipv4Addr>,
) -> Result<(), NodeCustomErrors> {
    let host = config.dns_seed.clone();
    let port = config.net_port;
    let addrs = (host, port)
        .to_socket_addrs()
        .map_err(|err| NodeCustomErrors::SocketError(err.to_string()))?;
    for addr in addrs {
        if let SocketAddr::V4(v4_addr) = addr {
            node_ips.push(*v4_addr.ip());
        }
    }
    write_in_log(
        &log_sender.info_log_sender,
        format!(
            "Se obtuvieron {} ips de la DNS: {:?}\n",
            node_ips.len(),
            node_ips
        )
        .as_str(),
    );
    Ok(())
}
