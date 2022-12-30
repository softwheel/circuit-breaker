use std::time::Duration;
use std::thread;
use std::sync::{mpsc, Arc, Mutex, RwLock};
use std::sync::atomic::{AtomicUsize, Ordering};

struct Timer {
    duration: Duration,
    tx: mpsc::SyncSender<()>,
}

impl Timer {
    fn new(duration: Duration, tx: mpsc::SyncSender<()>) -> Self {
        Timer { duration, tx }
    }

    fn start(&self) {
        let duration = self.duration;
        let tx = self.tx.clone();
        thread::spawn(move || {
            thread::sleep(duration);
            tx.send(()).unwrap();
        });
    }
}

enum State {
    // The circuit breaker is closed and allowing requests
    // to pass through
    Closed,
    // The circuit breaker is open and blocking requests
    Open,
    // The circuit breaker is half-open and allowing a limited
    // number of requests to pass through
    HalfOpen,
}

struct CircuitBreaker {
    state: Arc<RwLock<State>>,
    // The timer which will wait for trip_timeout duration before
    // transitioning from the open state to the half-open state
    trip_timer: Timer,
    trip_timer_receiver: Arc<Mutex<mpsc::Receiver<()>>>,
    // The maximum number of requests allowed through in
    // the closed state
    max_failures: usize,
    // The number of consecutive failures in the closed
    // state
    consecutive_failures: Arc<AtomicUsize>,
}

impl CircuitBreaker {
    pub fn new(max_failures: usize, trip_timeout: Duration) -> CircuitBreaker {
        let (tx, rx) = mpsc::sync_channel(1);
        CircuitBreaker {
            state: Arc::new(RwLock::new(State::Closed)),
            max_failures,
            trip_timer: Timer::new(trip_timeout, tx),
            trip_timer_receiver: Arc::new(Mutex::new(rx)),
            consecutive_failures: Arc::new(AtomicUsize::new(0)),
        }
    }
    
    pub fn call<F, T, E>(&mut self, f: F) -> Option<Result<T, E>>
    where
        F: FnOnce() -> Result<T, E>,
    {
        let mut state = self.state.write().unwrap();
        match *state {
            // If the circuit breaker is closed, try the request
            // and track the result
            State::Closed => {
                if self.consecutive_failures.load(Ordering::Relaxed) < self.max_failures {
                    let result = f();
                    if let Err(_) = result {
                        self.record_failure();
                    }
                    Some(result)
                } else {
                    *state = State::Open;
                    self.consecutive_failures.store(0, Ordering::Relaxed);
                    self.trip_timer.start();
                    self.spawn_trip_reset();
                    None
                }
            }
            // If the circuit breaker is open, check if it's time
            // to transition to the half-open state
            State::Open => {
                None
            }
            // If the circuit breaker is half-open, attempt a limited
            // number of requests to pass through
            State::HalfOpen => {
                let result = f();
                if let Err(_) = result {
                    *state = State::Open;
                    self.trip_timer.start();
                    self.spawn_trip_reset();
                } else {
                    *state = State::Closed;
                }
                Some(result)
            }
        }
    }

    fn spawn_trip_reset(&self) {
        let state_lock = self.state.clone();
        let rx = self.trip_timer_receiver.clone();
        thread::spawn(move || {
            let rx = rx.lock().unwrap();
            while let Ok(_) = rx.recv() {
                let mut state = state_lock.write().unwrap();
                *state = State::HalfOpen;
            };
        });
    }
    
    fn record_failure(&self) -> usize {
        let state = self.state.read().unwrap();
        match *state {
            State::Closed => self.consecutive_failures.fetch_add(1, Ordering::Relaxed),
            State::Open => 0,
            State::HalfOpen => self.consecutive_failures.fetch_add(1, Ordering::Relaxed),
        }
    }
}

fn request(dice: u32) -> Result<u32, String> {
    if dice > 6 {
        Err("400: Bad request.".to_string())
    } else {
        Ok(dice)
    }
}

fn main() {

    let mut cb = CircuitBreaker::new(3, Duration::from_secs(10));
    println!("Circuit Breaker has been set with");
    println!("    * 3 as maximum consecutive failures");
    println!("    * 10 seconds as the trip timeout");
    println!("");
    
    println!("Circuit Breaker is in the initial state, which is closed.");
    // The circuit breaker is in the closed state, so the function
    // will be executed
    let result = cb.call(|| request(5));
    println!("Result for request_dice(5): {:?}", result);
    
    println!("Circuit Breaker is encounting 3 errors in a row ...");
    // The function returns an error 3 times in a row, so the circuit
    // breaker transitions to the open state
    cb.call(|| request(10));
    cb.call(|| request(10));
    cb.call(|| request(10));
    
    // The circuit breaker is in the open state, so the function is
    // not executed
    let result = cb.call(|| request(2));
    println!("Result for request_dice(2): {:?}", result);
    
    // The circuit breaker is in the half-open state after trip_timeout
    // seconds, so the function will be executed
    println!("Let's have fun by doing nothing in 20 seconds :)");
    println!("...");
    thread::sleep(Duration::from_secs(20));
    let result = cb.call(|| request(5));
    println!("Result for request_dice(5): {:?}", result);
    let result = cb.call(|| request(6));
    println!("Result for request_dice(6): {:?}", result);
}
