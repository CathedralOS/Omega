use super::*;

fn transfer_budget() -> ResolverExecutionTransferBudget {
    ResolverExecutionTransferBudget::new(1024 * 1024).expect("construct transfer budget")
}

fn send_request(route: &ResolverExecutionEndpointRoute, request: &[u8]) -> Vec<u8> {
    let mut stream =
        TcpStream::connect(route.policy().broker_endpoint()).expect("connect to endpoint broker");
    stream.write_all(request).expect("write CONNECT request");
    stream
        .shutdown(Shutdown::Write)
        .expect("finish CONNECT request");
    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .expect("read proxy response");
    response
}

#[test]
fn requested_endpoint_is_typed_normalized_and_bounded() {
    let endpoint =
        ResolverExecutionRequestedEndpoint::new("GitHub.COM", 443).expect("normalize endpoint");
    assert_eq!(endpoint.authority(), "github.com:443");
    assert!(ResolverExecutionRequestedEndpoint::new("bad_name.example", 443).is_err());
    assert!(ResolverExecutionRequestedEndpoint::new("example.com", 0).is_err());
    assert!(ResolverExecutionRequestedEndpoint::new("2001:db8::1", 22).is_ok());
    assert!(ResolverExecutionTransferBudget::new(0).is_err());
}

#[test]
fn complete_dns_answer_is_bounded_before_connection_selection() {
    let addresses = (1..=DNS_RESULT_LIMIT + 1).map(|index| {
        SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            u16::try_from(index).expect("test port is bounded"),
        )
    });
    assert!(collect_bounded_addresses(addresses).is_err());
    let exact = (1..=DNS_RESULT_LIMIT).map(|index| {
        SocketAddr::new(
            IpAddr::V4(Ipv4Addr::LOCALHOST),
            u16::try_from(index).expect("test port is bounded"),
        )
    });
    assert_eq!(
        collect_bounded_addresses(exact)
            .expect("exact bounded DNS answer")
            .len(),
        DNS_RESULT_LIMIT
    );
}

#[test]
fn connect_helper_uses_only_the_broker_and_relays_the_tunnel() {
    let upstream = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind upstream canary");
    let upstream_endpoint = upstream.local_addr().expect("read upstream endpoint");
    let requested = ResolverExecutionRequestedEndpoint::new(
        &upstream_endpoint.ip().to_string(),
        upstream_endpoint.port(),
    )
    .expect("construct upstream endpoint");
    let budget = transfer_budget();
    let route = ResolverExecutionEndpointRoute::open(requested.clone(), budget.clone())
        .expect("open compiler endpoint route");
    let upstream_worker = thread::spawn(move || {
        let (mut connection, _) = upstream.accept().expect("accept routed connection");
        let mut request = [0_u8; 4];
        connection
            .read_exact(&mut request)
            .expect("read tunneled request");
        assert_eq!(&request, b"ping");
        connection.write_all(b"pong").expect("write tunneled reply");
    });

    let (mut tunnel, trailing) = open_connect_tunnel(
        route.policy().broker_endpoint(),
        route.policy().requested_endpoint(),
    )
    .expect("open CONNECT tunnel");
    assert!(trailing.is_empty());
    tunnel.write_all(b"ping").expect("write tunnel request");
    tunnel
        .shutdown(Shutdown::Write)
        .expect("finish tunnel request");
    let mut reply = Vec::new();
    tunnel.read_to_end(&mut reply).expect("read tunnel reply");
    assert_eq!(reply, b"pong");

    upstream_worker.join().expect("join upstream canary");
    let observation = route.finish().expect("finish endpoint route");
    let event = observation
        .events()
        .iter()
        .find(|event| event.outcome() == ResolverExecutionEndpointOutcome::Connected)
        .expect("retain connected event");
    assert_eq!(event.effective_peer(), Some(upstream_endpoint));
    assert_eq!(event.uploaded_bytes(), 4);
    assert_eq!(event.downloaded_bytes(), 4);
    assert_eq!(budget.observed(), 8);
}

#[test]
fn connect_helper_rejects_a_failed_proxy_response() {
    let broker = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind rejecting proxy");
    let broker_endpoint = broker.local_addr().expect("read proxy endpoint");
    let worker = thread::spawn(move || {
        let (mut connection, _) = broker.accept().expect("accept helper connection");
        let mut request = [0_u8; 512];
        let _ = connection.read(&mut request).expect("read CONNECT request");
        connection
            .write_all(b"HTTP/1.1 403 Forbidden\r\n\r\n")
            .expect("write rejecting response");
    });
    let requested = ResolverExecutionRequestedEndpoint::new("example.invalid", 22)
        .expect("construct requested endpoint");

    assert!(open_connect_tunnel(broker_endpoint, &requested).is_err());
    worker.join().expect("join rejecting proxy");
}

#[test]
fn broker_rejects_changed_malformed_and_oversized_destinations() {
    let destination = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind destination");
    let destination_address = destination.local_addr().expect("destination address");
    let endpoint = ResolverExecutionRequestedEndpoint::new(
        &destination_address.ip().to_string(),
        destination_address.port(),
    )
    .expect("destination endpoint");
    let route =
        ResolverExecutionEndpointRoute::open(endpoint, transfer_budget()).expect("open route");

    let changed_port = if destination_address.port() == u16::MAX {
        destination_address.port() - 1
    } else {
        destination_address.port() + 1
    };
    let changed = send_request(
        &route,
        format!("CONNECT 127.0.0.1:{changed_port} HTTP/1.1\r\n\r\n").as_bytes(),
    );
    assert!(changed.starts_with(b"HTTP/1.1 403"));
    let malformed = send_request(&route, b"GET / HTTP/1.1\r\n\r\n");
    assert!(malformed.starts_with(b"HTTP/1.1 400"));
    let mut oversized = b"CONNECT 127.0.0.1:1 HTTP/1.1\r\nX: ".to_vec();
    oversized.resize(CONNECT_REQUEST_BYTE_LIMIT + 1, b'a');
    let response = send_request(&route, &oversized);
    assert!(response.starts_with(b"HTTP/1.1 431"));

    let observation = route.finish().expect("finish route");
    assert_eq!(observation.events().len(), 3);
    assert_eq!(
        observation.events()[0].outcome(),
        ResolverExecutionEndpointOutcome::DestinationRejected
    );
    assert_eq!(
        observation.events()[1].outcome(),
        ResolverExecutionEndpointOutcome::MalformedConnect
    );
    assert_eq!(
        observation.events()[2].outcome(),
        ResolverExecutionEndpointOutcome::OversizedConnect
    );
    assert!(
        observation
            .events()
            .iter()
            .all(|event| event.effective_peer().is_none()
                && event.uploaded_bytes() == 0
                && event.downloaded_bytes() == 0)
    );
}

#[test]
fn broker_records_the_effective_connected_peer() {
    let destination = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind destination");
    let destination_address = destination.local_addr().expect("destination address");
    let acceptance = thread::spawn(move || {
        let (mut stream, _) = destination.accept().expect("accept broker connection");
        let mut byte = [0_u8; 1];
        stream.read_exact(&mut byte).expect("read relayed byte");
        stream.write_all(&byte).expect("echo relayed byte");
    });
    let endpoint = ResolverExecutionRequestedEndpoint::new(
        &destination_address.ip().to_string(),
        destination_address.port(),
    )
    .expect("destination endpoint");
    let budget = transfer_budget();
    let route =
        ResolverExecutionEndpointRoute::open(endpoint.clone(), budget.clone()).expect("open route");
    let mut client = TcpStream::connect(route.policy().broker_endpoint()).expect("connect proxy");
    write!(
        client,
        "CONNECT {} HTTP/1.1\r\nHost: {}\r\n\r\n",
        endpoint.authority(),
        endpoint.authority()
    )
    .expect("write CONNECT");
    let mut response = [0_u8; 39];
    client
        .read_exact(&mut response)
        .expect("read CONNECT response");
    assert_eq!(&response, b"HTTP/1.1 200 Connection Established\r\n\r\n");
    client.write_all(b"x").expect("write tunneled byte");
    let mut echoed = [0_u8; 1];
    client.read_exact(&mut echoed).expect("read tunneled byte");
    assert_eq!(echoed, *b"x");
    drop(client);
    acceptance.join().expect("destination worker");

    let observation = route.finish().expect("finish route");
    assert_eq!(observation.events().len(), 1);
    assert_eq!(
        observation.events()[0].outcome(),
        ResolverExecutionEndpointOutcome::Connected
    );
    assert_eq!(
        observation.events()[0].effective_peer(),
        Some(destination_address)
    );
    assert_eq!(observation.events()[0].uploaded_bytes(), 1);
    assert_eq!(observation.events()[0].downloaded_bytes(), 1);
    assert_eq!(budget.observed(), 2);
    assert_eq!(observation.route().requested_endpoint(), &endpoint);
    assert_eq!(
        observation.canonical_bytes(),
        observation.clone().canonical_bytes()
    );
}

#[test]
fn broker_acceptance_and_observation_are_connection_bounded() {
    let endpoint =
        ResolverExecutionRequestedEndpoint::new("127.0.0.1", 9).expect("discard endpoint");
    let route =
        ResolverExecutionEndpointRoute::open(endpoint, transfer_budget()).expect("open route");
    for _ in 0..ENDPOINT_CONNECTION_LIMIT {
        let response = send_request(&route, b"GET / HTTP/1.1\r\n\r\n");
        assert!(response.starts_with(b"HTTP/1.1 400"));
    }
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        match TcpStream::connect(route.policy().broker_endpoint()) {
            Err(_) => break,
            Ok(stream) => drop(stream),
        }
        assert!(
            Instant::now() < deadline,
            "broker retained an unbounded accept surface"
        );
        thread::sleep(IO_POLL_INTERVAL);
    }
    let observation = route.finish().expect("finish route");
    assert_eq!(
        observation.events().len(),
        usize::from(ENDPOINT_CONNECTION_LIMIT)
    );
    assert_eq!(
        observation.events().last().expect("last event").ordinal(),
        ENDPOINT_CONNECTION_LIMIT
    );
}

#[test]
fn shared_transfer_budget_is_atomic_and_never_overshoots() {
    let budget = ResolverExecutionTransferBudget::new(100).expect("construct tight budget");
    let workers = (0..8)
        .map(|_| {
            let budget = budget.clone();
            thread::spawn(move || (0..100).filter(|_| budget.charge(1).is_ok()).count())
        })
        .collect::<Vec<_>>();
    let charged = workers
        .into_iter()
        .map(|worker| worker.join().expect("join charging worker"))
        .sum::<usize>();
    assert_eq!(charged, 100);
    assert_eq!(budget.observed(), budget.ceiling());
    assert!(budget.charge(1).is_err());
}

#[test]
fn transfer_ceiling_is_shared_across_routes_and_recorded_conservatively() {
    let upstream = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind upstream");
    let upstream_endpoint = upstream.local_addr().expect("read upstream endpoint");
    let endpoint = ResolverExecutionRequestedEndpoint::new(
        &upstream_endpoint.ip().to_string(),
        upstream_endpoint.port(),
    )
    .expect("construct endpoint");
    let acceptance = thread::spawn(move || {
        for _ in 0..2 {
            let (mut connection, _) = upstream.accept().expect("accept routed connection");
            let mut request = Vec::new();
            connection
                .read_to_end(&mut request)
                .expect("read routed request");
        }
    });
    let budget = ResolverExecutionTransferBudget::new(6).expect("construct tight budget");

    let first = ResolverExecutionEndpointRoute::open(endpoint.clone(), budget.clone())
        .expect("open first route");
    let (mut tunnel, _) =
        open_connect_tunnel(first.policy().broker_endpoint(), &endpoint).expect("open tunnel");
    tunnel.write_all(b"four").expect("write first request");
    tunnel
        .shutdown(Shutdown::Write)
        .expect("finish first request");
    let mut response = Vec::new();
    tunnel
        .read_to_end(&mut response)
        .expect("close first tunnel");
    let first = first.finish().expect("finish first route");
    assert_eq!(
        first.events()[0].outcome(),
        ResolverExecutionEndpointOutcome::Connected
    );
    assert_eq!(first.events()[0].uploaded_bytes(), 4);
    assert_eq!(budget.observed(), 4);

    let second = ResolverExecutionEndpointRoute::open(endpoint.clone(), budget.clone())
        .expect("open second route");
    let (mut tunnel, _) = open_connect_tunnel(second.policy().broker_endpoint(), &endpoint)
        .expect("open second tunnel");
    tunnel.write_all(b"four").expect("write rejected request");
    tunnel
        .shutdown(Shutdown::Write)
        .expect("finish rejected request");
    let mut response = Vec::new();
    tunnel
        .read_to_end(&mut response)
        .expect("close rejected tunnel");
    let second = second.finish().expect("finish second route");
    assert_eq!(
        second.events()[0].outcome(),
        ResolverExecutionEndpointOutcome::TransferCeilingReached
    );
    assert_eq!(second.events()[0].uploaded_bytes(), 0);
    assert_eq!(second.events()[0].downloaded_bytes(), 0);
    assert_eq!(budget.observed(), 4);
    acceptance.join().expect("join upstream worker");
}

#[test]
fn route_policy_identity_binds_the_transfer_ceiling() {
    let endpoint = ResolverExecutionRequestedEndpoint::new("127.0.0.1", 443).expect("endpoint");
    let broker_endpoint = SocketAddr::from((Ipv4Addr::LOCALHOST, 12345));
    let first = ResolverExecutionEndpointRoutePolicy {
        requested_endpoint: endpoint.clone(),
        broker_endpoint,
        connection_limit: ENDPOINT_CONNECTION_LIMIT,
        transfer_byte_ceiling: 10,
    };
    let mut second = first.clone();
    second.transfer_byte_ceiling = 11;
    let mut first_bytes = Vec::new();
    first.encode(&mut first_bytes);
    let mut second_bytes = Vec::new();
    second.encode(&mut second_bytes);
    assert_ne!(first_bytes, second_bytes);
}
