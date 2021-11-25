use std::time::Duration;

use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;
use tokio::runtime::Runtime;

use identity::account::Account;
use identity::account::AccountBuilder;
use identity::account::AccountStorage;
use identity::account::AutoSave;
use identity::account::IdentitySetup;

const ACTIONS: usize = 10;
const AUTOSAVE_SETTINGS: [AutoSave; 4] = [AutoSave::Never, AutoSave::Every, AutoSave::Batch(2), AutoSave::Batch(5)];
const PASSWORD: &'static str = "my-password";
const STONGHOLD_PATH: &'static str = "./example-strong.hodl";
const SAMPLE_SIZE: usize = 10;

fn bench_autosave(c: &mut Criterion) {
  let rt = Runtime::new().unwrap();

  let mut group = c.benchmark_group(format!("Autosave Setting - Number of Actions: {}", ACTIONS));
  group.sample_size(SAMPLE_SIZE);
  group.measurement_time(Duration::from_secs(1_000));
  for setting in AUTOSAVE_SETTINGS {
    group.bench_with_input(BenchmarkId::new(format!("{:?}", setting), ACTIONS), &ACTIONS, |b, n| {
      b.to_async(&rt)
        .iter(|| async { multiple_identity_updates(setting, *n).await })
    });
  }
  group.finish();
}

async fn multiple_identity_updates(auto_save: AutoSave, n: usize) {
  let mut builder: AccountBuilder =
    Account::builder()
      .autopublish(false)
      .autosave(auto_save)
      .storage(AccountStorage::Stronghold(
        STONGHOLD_PATH.into(),
        Some(PASSWORD.into()),
        Some(false),
      ));
  let mut account1: Account = builder.create_identity(IdentitySetup::default()).await.unwrap();

  for i in 0..n {
    account1
      .update_identity()
      .create_method()
      .fragment(format!("my-key-{}", i))
      .apply()
      .await
      .unwrap();
    println!("{}", i);
  }
}

criterion_group!(benches, bench_autosave);
criterion_main!(benches);
