use std::time::{Duration, Instant};
use std::thread;

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
    state: State,
    // The duration to wait before transitioning from the
    // open state to the half-open state
    trip_timeout: Duration,
    // The maximum number of requests allowed through in
    // the closed state
    max_failures: usize,
    // The number of consecutive failures in the closed
    // state
    consecutive_failures: usize,
    // The time when the circuit breaker transitioned to the
    // open state
    opened_at: Option<Instant>,
}

impl CircuitBreaker {
    pub fn new(max_failures: usize, trip_timeout: Duration) -> CircuitBreaker {
        CircuitBreaker {
            state: State::Closed,
            max_failures,
            trip_timeout,
            consecutive_failures: 0,
            opened_at: None,
        }
    }
    
    pub fn call<F, T, E>(&mut self, f: F) -> Option<Result<T, E>>
    where
        F: FnOnce() -> Result<T, E>,
    {
        match self.state {
            // If the circuit breaker is closed, try the request
            // and track the result
            State::Closed => {
                if self.consecutive_failures < self.max_failures {
                    let result = f();
                    if let Err(_) = result {
                        self.record_failure();
                    }
                    Some(result)
                } else {
                    self.opened_at = Some(Instant::now());
                    self.state = State::Open;
                    self.consecutive_failures = 0;
                    None
                }
            }
            // If the circuit breaker is open, check if it's time
            // to transition to the half-open state
            State::Open => {
                if let Some(opened_at) = self.opened_at {
                    let elapsed = Instant::now().duration_since(opened_at);
                    if elapsed >= self.trip_timeout {
                        self.state = State::HalfOpen;
                        self.opened_at = None;
                    }
                }
                None
            }
            // If the circuit breaker is half-open, attempt a limited
            // number of requests to pass through
            State::HalfOpen => {
                let result = f();
                if let Err(_) = result {
                    self.state = State::Open;
                } else {
                    self.state = State::Closed;
                }
                Some(result)
            }
        }
    }
    
    fn record_failure(&mut self) {
        match self.state {
            State::Closed => self.consecutive_failures += 1,
            State::Open => (),
            State::HalfOpen => self.consecutive_failures += 1,
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
