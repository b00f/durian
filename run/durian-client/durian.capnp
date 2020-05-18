@0xd767186f03554834;



struct Transaction {
  sender @0: Data;
  value @1: Data;
  gas @2: Data;
  gasPrice @3: Data;
  action: union{
    create: group {
      code @4: Data;
      salt @5: Data;
    }
    call: group {
      address @6: Data;
    }
  }
  args @7: Data;
}

struct LogEntry {
  address @0: Data;
  topics @1: List(Data);
  data @2: List(Int8);
}

struct ResultData {
  gasLeft @0: Data;
  data @1: List(Int8);
  contract @2: Data;
  logs @3: List(LogEntry);
}

interface Executor {
  execute @0 (provider: Provider, transaction: Transaction) -> (resultData: ResultData);
}

interface Provider {
  getStorage @0 (address: Data) -> (storage: Data);
  setStorage @1 (address: Data, storage: Data) -> ();
}
