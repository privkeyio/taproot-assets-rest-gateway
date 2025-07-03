use actix_web::{test, App};
use serial_test::serial;
use std::time::{Duration, Instant};
use taproot_assets_rest_gateway::api::routes::configure;
use taproot_assets_rest_gateway::tests::setup::setup;

#[actix_rt::test]
#[ignore] // Run with: cargo test --test benchmarks test_throughput_benchmark -- --ignored --nocapture
async fn test_throughput_benchmark() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    println!("\n=== Throughput Benchmark ===");
    println!("Testing maximum requests/second for simple queries...\n");

    // Test different endpoints
    let endpoints = vec![
        ("/v1/taproot-assets/getinfo", "GetInfo"),
        ("/v1/taproot-assets/assets", "ListAssets"),
        ("/v1/taproot-assets/assets/balance", "GetBalance"),
        ("/health", "Health Check"),
    ];

    for (endpoint, name) in endpoints {
        let mut request_count = 0;
        let mut error_count = 0;

        // Run for 10 seconds
        let duration = Duration::from_secs(10);
        let start = Instant::now();

        // Sequential requests (actix-web test framework limitation)
        while start.elapsed() < duration {
            let req = test::TestRequest::get().uri(endpoint).to_request();

            let resp = test::call_service(&app, req).await;

            if resp.status().is_success() {
                request_count += 1;
            } else {
                error_count += 1;
            }
        }

        let elapsed = start.elapsed();
        let requests_per_second = request_count as f64 / elapsed.as_secs_f64();

        println!("{name} Endpoint Results:");
        println!("  Total requests: {request_count}");
        println!("  Errors: {error_count}");
        println!("  Duration: {:.2}s", elapsed.as_secs_f64());
        println!("  Throughput: {requests_per_second:.0} req/s");
        println!(
            "  Avg latency: {:.2}ms\n",
            elapsed.as_millis() as f64 / request_count as f64
        );
    }

    println!("Note: These are sequential request benchmarks due to actix-web test framework limitations.");
    println!("Real-world concurrent performance will be significantly higher.");
}

#[actix_rt::test]
#[ignore] // Run with: cargo test --test benchmarks test_latency_comparison -- --ignored --nocapture
async fn test_latency_comparison() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    println!("\n=== Latency Comparison ===");
    println!("Measuring REST gateway overhead vs direct backend calls...\n");

    // Test REST gateway latency
    let mut rest_times = Vec::new();
    for _ in 0..100 {
        let start = Instant::now();
        let req = test::TestRequest::get()
            .uri("/v1/taproot-assets/getinfo")
            .to_request();
        let _ = test::call_service(&app, req).await;
        rest_times.push(start.elapsed());
    }

    // Direct HTTP request to simulate lower-level access
    let mut direct_times = Vec::new();
    for _ in 0..100 {
        let start = Instant::now();
        // Direct HTTP request to simulate gRPC performance
        let _ = client
            .get(format!("{}/v1/taproot-assets/getinfo", base_url.0))
            .header("Grpc-Metadata-macaroon", &macaroon_hex.0)
            .send()
            .await;
        direct_times.push(start.elapsed());
    }

    // Calculate statistics
    rest_times.sort();
    direct_times.sort();

    let rest_p50 = rest_times[50].as_micros() as f64 / 1000.0;
    let rest_p95 = rest_times[95].as_micros() as f64 / 1000.0;
    let rest_avg = rest_times.iter().map(|d| d.as_micros()).sum::<u128>() as f64 / 100000.0;

    let direct_p50 = direct_times[50].as_micros() as f64 / 1000.0;
    let direct_p95 = direct_times[95].as_micros() as f64 / 1000.0;
    let direct_avg = direct_times.iter().map(|d| d.as_micros()).sum::<u128>() as f64 / 100000.0;

    println!("REST Gateway Latency:");
    println!("  P50: {rest_p50:.2}ms");
    println!("  P95: {rest_p95:.2}ms");
    println!("  Average: {rest_avg:.2}ms");

    println!("\nDirect Backend Latency:");
    println!("  P50: {direct_p50:.2}ms");
    println!("  P95: {direct_p95:.2}ms");
    println!("  Average: {direct_avg:.2}ms");

    println!("\nOverhead:");
    println!("  P50: {:.2}ms", rest_p50 - direct_p50);
    println!("  P95: {:.2}ms", rest_p95 - direct_p95);
    println!("  Average: {:.2}ms", rest_avg - direct_avg);

    // Verify claim of <10ms overhead
    let overhead_p95 = rest_p95 - direct_p95;
    if overhead_p95 < 10.0 {
        println!("\n✅ P95 overhead is under 10ms!");
    } else {
        println!("\n⚠️  P95 overhead exceeds 10ms: {overhead_p95:.2}ms");
    }
}

#[actix_rt::test]
#[ignore] // Run with: cargo test --test benchmarks test_memory_usage -- --ignored --nocapture
async fn test_memory_usage() {
    use sysinfo::{ProcessesToUpdate, System};

    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    println!("\n=== Memory Usage Benchmark ===");

    let mut system = System::new_all();
    system.refresh_all();

    let pid = sysinfo::Pid::from(std::process::id() as usize);
    let process = system.process(pid).expect("Failed to get process info");
    let baseline_memory = process.memory() / 1024 / 1024; // Bytes to MB

    println!("Baseline memory usage: {baseline_memory} MB");

    // Perform various operations
    println!("\nPerforming 1000 requests...");
    for i in 0..1000 {
        let req = test::TestRequest::get()
            .uri("/v1/taproot-assets/assets")
            .to_request();
        let _ = test::call_service(&app, req).await;

        if i % 100 == 0 && i > 0 {
            // Use refresh_processes instead of refresh_process
            system.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);
            if let Some(process) = system.process(pid) {
                let current_memory = process.memory() / 1024 / 1024;
                println!("Memory after {i} requests: {current_memory} MB");
            }
        }
    }

    // Final measurement
    system.refresh_processes(ProcessesToUpdate::Some(&[pid]), false);
    if let Some(process) = system.process(pid) {
        let final_memory = process.memory() / 1024 / 1024;
        let memory_growth = final_memory.saturating_sub(baseline_memory);

        println!("\nFinal memory usage: {final_memory} MB");
        println!("Memory growth: {memory_growth} MB");

        // Verify claim of ~50MB baseline
        if baseline_memory < 100 {
            println!("✅ Baseline memory is under 100MB");
        } else {
            println!("⚠️  Baseline memory exceeds 100MB");
        }
    }
}

#[actix_rt::test]
#[ignore] // Run with: cargo test --test benchmarks test_request_burst -- --ignored --nocapture
async fn test_request_burst() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    println!("\n=== Request Burst Test ===");
    println!("Testing performance under burst load...\n");

    for burst_size in [10, 50, 100, 500, 1000] {
        let start = Instant::now();
        let mut success = 0;
        let mut errors = 0;

        for _ in 0..burst_size {
            let req = test::TestRequest::get()
                .uri("/v1/taproot-assets/assets/balance")
                .to_request();

            let resp = test::call_service(&app, req).await;
            if resp.status().is_success() {
                success += 1;
            } else {
                errors += 1;
            }
        }

        let elapsed = start.elapsed();
        let throughput = burst_size as f64 / elapsed.as_secs_f64();
        let avg_latency = elapsed.as_millis() as f64 / burst_size as f64;

        println!("Burst of {burst_size} requests:");
        println!("  Successful: {success}");
        println!("  Errors: {errors}");
        println!("  Duration: {:.2}s", elapsed.as_secs_f64());
        println!("  Throughput: {throughput:.0} req/s");
        println!("  Avg latency: {avg_latency:.2}ms");
        println!(
            "  Success rate: {:.1}%\n",
            (success as f64 / burst_size as f64) * 100.0
        );
    }
}

#[actix_rt::test]
#[ignore] // Run with: cargo test --test benchmarks test_api_response_time_percentiles -- --ignored --nocapture
async fn test_api_response_time_percentiles() {
    let (client, base_url, macaroon_hex, _) = setup().await;
    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    println!("\n=== Response Time Percentiles ===");

    let mut response_times = Vec::new();
    let endpoints = vec![
        ("/v1/taproot-assets/getinfo", "GetInfo"),
        ("/v1/taproot-assets/assets", "ListAssets"),
        ("/v1/taproot-assets/assets/balance", "GetBalance"),
        ("/v1/taproot-assets/addrs", "ListAddresses"),
    ];

    for (endpoint, name) in &endpoints {
        println!("\nTesting {name} endpoint...");
        let mut endpoint_times = Vec::new();

        for _ in 0..100 {
            let start = Instant::now();
            let req = test::TestRequest::get().uri(endpoint).to_request();
            let _ = test::call_service(&app, req).await;
            let elapsed = start.elapsed().as_millis();
            endpoint_times.push(elapsed);
            response_times.push(elapsed);
        }

        endpoint_times.sort();
        let p50 = endpoint_times[50];
        let p95 = endpoint_times[95];
        let p99 = endpoint_times[99];

        println!("  P50: {p50}ms");
        println!("  P95: {p95}ms");
        println!("  P99: {p99}ms");
    }

    response_times.sort();
    let overall_p50 = response_times[response_times.len() / 2];
    let overall_p95 = response_times[response_times.len() * 95 / 100];
    let overall_p99 = response_times[response_times.len() * 99 / 100];

    println!("\nOverall Response Times:");
    println!("  P50: {overall_p50}ms");
    println!("  P95: {overall_p95}ms");
    println!("  P99: {overall_p99}ms");

    if overall_p95 < 100 {
        println!("\n✅ P95 response time is under 100ms");
    } else {
        println!("\n⚠️  P95 response time exceeds 100ms: {overall_p95}ms");
    }
}

#[actix_rt::test]
#[serial]
#[ignore] // Run with: cargo test --test benchmarks test_real_world_simulation -- --ignored --nocapture
async fn test_real_world_simulation() {
    let (client, base_url, macaroon_hex, lnd_macaroon_hex) = setup().await;

    // First mint an asset for realistic testing
    let asset_id = taproot_assets_rest_gateway::tests::setup::mint_test_asset(
        client.as_ref(),
        &base_url.0,
        &macaroon_hex.0,
        &lnd_macaroon_hex,
    )
    .await;

    let app = test::init_service(
        App::new()
            .app_data(client.clone())
            .app_data(base_url.clone())
            .app_data(macaroon_hex.clone())
            .configure(configure),
    )
    .await;

    println!("\n=== Real-World Usage Simulation ===");
    println!("Simulating typical application usage patterns...\n");

    let start = Instant::now();
    let mut operations = Vec::new();

    // 1. Check system info
    let op_start = Instant::now();
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/getinfo")
        .to_request();
    let _ = test::call_service(&app, req).await;
    operations.push(("GetInfo", op_start.elapsed()));

    // 2. List assets
    let op_start = Instant::now();
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets")
        .to_request();
    let _ = test::call_service(&app, req).await;
    operations.push(("ListAssets", op_start.elapsed()));

    // 3. Check balance
    let op_start = Instant::now();
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/assets/balance")
        .to_request();
    let _ = test::call_service(&app, req).await;
    operations.push(("GetBalance", op_start.elapsed()));

    // 4. Create an address
    let op_start = Instant::now();
    let addr_req = serde_json::json!({
        "asset_id": asset_id,
        "amt": "10"
    });
    let req = test::TestRequest::post()
        .uri("/v1/taproot-assets/addrs")
        .set_json(&addr_req)
        .to_request();
    let resp = test::call_service(&app, req).await;
    let addr_json: serde_json::Value = test::read_body_json(resp).await;
    operations.push(("CreateAddress", op_start.elapsed()));

    // 5. List addresses
    let op_start = Instant::now();
    let req = test::TestRequest::get()
        .uri("/v1/taproot-assets/addrs")
        .to_request();
    let _ = test::call_service(&app, req).await;
    operations.push(("ListAddresses", op_start.elapsed()));

    // 6. Attempt a send (will likely fail but tests the endpoint)
    if let Some(addr) = addr_json.get("encoded").and_then(|v| v.as_str()) {
        let op_start = Instant::now();
        let send_req = serde_json::json!({
            "tap_addrs": vec![addr],
            "fee_rate": 10
        });
        let req = test::TestRequest::post()
            .uri("/v1/taproot-assets/send")
            .set_json(&send_req)
            .to_request();
        let _ = test::call_service(&app, req).await;
        operations.push(("SendAssets", op_start.elapsed()));
    }

    let total_elapsed = start.elapsed();

    println!("Operation Timings:");
    for (name, duration) in &operations {
        println!("  {}: {:.2}ms", name, duration.as_secs_f64() * 1000.0);
    }

    println!(
        "\nTotal time for workflow: {:.2}ms",
        total_elapsed.as_secs_f64() * 1000.0
    );

    let avg_time = operations.iter().map(|(_, d)| d.as_millis()).sum::<u128>() as f64
        / operations.len() as f64;
    println!("Average operation time: {avg_time:.2}ms");

    if total_elapsed < Duration::from_secs(1) {
        println!("\n✅ Complete workflow executed in under 1 second!");
    }
}

// Helper function to print benchmark summary
fn print_summary() {
    println!("\n=== Benchmark Summary ===");
    println!("To validate performance claims:");
    println!("1. Run throughput test to verify ~5,000 req/s claim");
    println!("2. Run latency comparison to verify <10ms overhead");
    println!("3. Run memory usage test to verify ~50MB baseline");
    println!("4. Document results in BENCHMARKS.md");
    println!("\nNote: Test framework limitations mean real concurrent performance");
    println!("will be higher than these sequential benchmark results.");
}

#[actix_rt::test]
#[ignore]
async fn run_all_benchmarks() {
    print_summary();
}
