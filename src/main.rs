mod tasks_handler;

use tasks_handler::Threadpool;

fn main() {
    let tasks_number: usize;
    let workers_number: u8;

    {
        let mut args: Vec<String> = std::env::args().skip(1).collect();
        assert!(args.len() == 2, "App requires two arguments. First: number of tasks which will be generated. Second: number of workers which will handle these tasks.");
        workers_number = args.pop().expect("There should be cmd argument which specifies workers number").parse::<u8>().unwrap();
        tasks_number = args.pop().expect("There should be cmd argument which specifies task number").parse::<usize>().unwrap();
    }

    assert!(tasks_number > 0, "Tasks number must be higher than 0");
    assert!(workers_number > 0, "Workers number must be higher than 0");

    println!("Config:\nTasks to be generated: {},\nWorkers count: {}", tasks_number, workers_number);
    let mut tp = Threadpool::prepare(tasks_number, workers_number);
    tp.run();
}
