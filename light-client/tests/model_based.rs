use serde::Deserialize;
use tendermint_light_client::{
    tests::{Trusted, *},
    types::{LightBlock, Time, TrustThreshold},
};
use std::time::Duration;
use tendermint_testgen::{apalache::*, jsonatr::*, Command, Tester, TestEnv};
use std::{fs, path::PathBuf};


#[derive(Deserialize, Clone, Debug, PartialEq)]
pub enum LiteTestKind {
    SingleStep,
    Bisection
}

/// An abstraction of the LightClient verification verdict
#[derive(Deserialize, Clone, Debug, PartialEq)]
pub enum LiteVerdict {
    /// verified successfully
    OK,
    /// outside of trusting period
    #[serde(rename = "FAILED_TRUSTING_PERIOD")]
    FailedTrustingPeriod,
    /// block verification based on the header and commit structure failed
    #[serde(rename = "FAILED_VERIFICATION")]
    FailedVerification,
    /// passed block verification, but the validator set is too different to verify it
    #[serde(rename = "CANNOT_VERIFY")]
    CannotVerify
}

/// A single-step test case is a test for `Verifier::verify()` function.
/// It contains an initial trusted block, plus a sequence of input blocks,
/// each with the expected verdict.
/// The trusted state is to be updated only if the verdict is "OK"
#[derive(Deserialize, Clone, Debug)]
pub struct SingleStepTestCase {
    description: String,
    initial: Initial,
    input: Vec<BlockVerdict>,
}

/// A LiteBlock together with the time when it's being checked, and the expected verdict
#[derive(Deserialize, Clone, Debug)]
pub struct BlockVerdict {
    block: AnonLightBlock,
    now: Time,
    verdict: LiteVerdict,
}

fn single_step_test(tc: SingleStepTestCase) {
    let mut latest_trusted = Trusted::new(
        tc.initial.signed_header.clone(),
        tc.initial.next_validator_set.clone(),
    );
    let clock_drift = Duration::from_secs(1);
    let trusting_period: Duration = tc.initial.trusting_period.into();
    for (i, input) in tc.input.iter().enumerate() {
        println!("    > step {}, expecting {:?}", i, input.verdict);
        let now = input.now;
        match verify_single(
            latest_trusted.clone(),
            input.block.clone().into(),
            TrustThreshold::default(),
            trusting_period,
            clock_drift,
            now,
        ) {
                    Ok(new_state) => {
                        assert_eq!(input.verdict, LiteVerdict::OK);
                        let expected_state: LightBlock = input.block.clone().into();
                        assert_eq!(new_state, expected_state);
                        latest_trusted = Trusted::new(new_state.signed_header, new_state.next_validators);
                    }
                    Err(e) => {
                        eprintln!("      > lite: {:?}", e);
                        assert_ne!(input.verdict, LiteVerdict::OK);
                    }
        }
    }
}

fn check_program(program: &str) -> bool {
    if !Command::exists_program(program) {
        println!("  > {} not found", program);
        return false
    }
    true
}

fn model_based_test(test: ApalacheTestCase, env: &TestEnv, root_env: &TestEnv, output_env: &TestEnv) {
    if !check_program("tendermint-testgen") ||
       !check_program("apalache-mc") ||
       !check_program("jsonatr") {
       return
    }
    output_env.cleanup();
    env.copy_file_from_env(root_env, "Lightclient_A_1.tla");
    env.copy_file_from_env(root_env, "Blockchain_A_1.tla");
    env.copy_file_from_env(root_env, "LightTests.tla");
    env.copy_file_from_env(root_env, &test.model);
    println!("  > running Apalache...");
    let apalache_run = run_apalache_test(env.current_dir(), test);
    assert!(apalache_run.is_ok());
    assert!(apalache_run.unwrap().stdout.contains("The outcome is: Error"),
     "Apalache failed to generate a counterexample; please check the model, the test, and the length bound");

    let transform_spec = root_env.full_canonical_path("_jsonatr-lib/apalache_to_lite_test.json").unwrap();
    let transform = JsonatrTransform {
        input: "counterexample.json".to_string(),
        include: vec![transform_spec],
        output: "test.json".to_string()
    };
    let jsonatr_run = run_jsonatr_transform(env.current_dir(), transform);
    if let Err(e) = jsonatr_run {
        println!("Error jsonatr: {}", e)
    }


    let tc = env.parse_file_as::<SingleStepTestCase>("test.json").unwrap();
    println!("  > running auto-generated test...");
    single_step_test(tc);
    output_env.copy_file_from_env(env, "counterexample.tla");
    output_env.copy_file_from_env(env, "counterexample.json");
}

fn model_based_test_batch(batch: ApalacheTestBatch, env: &TestEnv, root_env: &TestEnv, output_env: &TestEnv) {
    for test in batch.tests {
        let tc = ApalacheTestCase {
            model: batch.model.clone(),
            test: test.clone(),
            length: batch.length,
            timeout: batch.timeout
        };
        println!("  Running model-based single-step test case: {}", test);

        model_based_test(tc, env, root_env, &output_env.push(&test).unwrap());
    }
}

const TEST_DIR: &str = "./tests/support/model_based";

#[test]
fn run_single_step_tests() {
    let mut tester = Tester::new("single_step", TEST_DIR);
    tester.add_test("static model-based single-step test", single_step_test);
    tester.add_test_with_env("full model-based single-step test", model_based_test);
    tester.add_test_with_env("full model-based single-step test batch", model_based_test_batch);

    //tester.run_for_file("first-model-based-test.json");
    tester.run_foreach_in_dir("");
    tester.print_results();
}
