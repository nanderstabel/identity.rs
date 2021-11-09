use std::time::Duration;

use criterion::criterion_group;
use criterion::criterion_main;
use criterion::BenchmarkId;
use criterion::Criterion;
use tokio::runtime::Runtime;

use identity::account::Account;
use identity::account::AccountStorage;
use identity::account::AutoSave;
use identity::account::IdentityCreate;

const AUTOSAVE_SETTINGS: [AutoSave; 4] = [
  AutoSave::Never,
  AutoSave::Every,
  AutoSave::Batch(2),
  AutoSave::Batch(6),
];
const PASSWORD: &'static str = "my-password";
const STONGHOLD_PATH: &'static str = "./example-strong.hodl";
const SAMPLE_SIZE: usize = 10;

fn bench_autosave(c: &mut Criterion) {
  let rt = Runtime::new().unwrap();

  for i in [6] {
    let mut group = c.benchmark_group(format!("Autosave Setting - Creating {} Identities", i));
    group.sample_size(SAMPLE_SIZE);
    group.measurement_time(Duration::from_secs(1_000));
    for setting in AUTOSAVE_SETTINGS {
      group.bench_with_input(BenchmarkId::new(format!("{:?}", setting), i), &i, |b, i| {
        b.to_async(&rt)
          .iter(|| async { create_multiple_identities(setting, *i).await })
      });
    }
    group.finish();
  }
}

async fn create_multiple_identities(auto_save: AutoSave, n: usize) {
  let account: Account = Account::builder()
    .autosave(auto_save)
    .autopublish(false)
    .storage(AccountStorage::Stronghold(STONGHOLD_PATH.into(), Some(PASSWORD.into())))
    .build()
    .await
    .unwrap();
  for i in 0..n {
    println!("\n{}", i);
    let _ = account.create_identity(IdentityCreate::default()).await.unwrap();
  }
}

criterion_group!(benches, bench_autosave);
criterion_main!(benches);
