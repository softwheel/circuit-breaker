use std::time::{Duration, Instant};
use std::thread;
use std::sync::{Arc, Mutex};

/// A `CircuitBreaker`'s error.
#[derive(Debug)]
enum Error<E> {
    /// An error from inner call.
    Inner(E),
    /// An error when call was rejected. 
    Rejected,
}

trait CircuitBreaker {
    /// Ask permission to call.
    ///
    /// Return:
    ///     `true` if a call is allowed.
    ///     `false` if a call is prohibited.
    fn is_call_permitted(&self) -> bool;

    /// Call a given function within Circuit Breaker.
    ///
    /// Depending on the excution result, the call will be recorded as success or failure.
    fn call<F, T, E>(&self, f: F) -> Result<T, Error<E>>
    where
        F: FnOnce() -> Result<T, E>;
}

impl CircuitBreaker for StateMachine {
    fn is_call_permitted(&self) -> bool {
        self.is_call_permitted()
    }

    fn call<F, T, E>(&self, f: F) -> Result<T, Error<E>>
    where
        F: FnOnce() -> Result<T, E>,
    {
        if !self.is_call_permitted() {
            return Err(Error::Rejected);
        }

        match f() {
            Ok(ok) => {
                self.on_success();
                Ok(ok)
            }
            Err(err) => {
                self.on_error();
                Err(Error::Inner(err))
            }
        }
    }
}

#[derive(Debug)]
enum State {
    // The circuit breaker is closed and allowing requests to pass through.
    Closed,
    // The circuit breaker is open and blocking requests until the trip duration expired.
    Open(Instant, Duration),
    // The circuit breaker is half-open after waiting for the trip duration and
    // will allow requests to pass through. The state keeps the previous duration
    // in an open state.
    HalfOpen(Duration),
}

struct Shared {
    state: State,
    consecutive_failures: u8,
}

struct Inner {
    shared: Mutex<Shared>,
}

struct StateMachine {
    inner: Arc<Inner>,
    max_failures: u8,
    trip_timeout: Duration,
}

impl Shared {
    fn transit_to_closed(&mut self) {
        self.state = State::Closed;
        self.consecutive_failures = 0;
    }

    fn transit_to_half_open(&mut self, delay: Duration) {
        self.state = State::HalfOpen(delay);
    }

    fn transit_to_open(&mut self, delay: Duration) {
        let until = Instant::now() + delay;
        self.state = State::Open(until, delay);
    }
}

impl StateMachine {
    fn new(max_failures: u8, trip_timeout: Duration) -> Self {
        StateMachine {
            inner: Arc::new(Inner {
                shared: Mutex::new(Shared {
                    state: State::Closed,
                    consecutive_failures: 0,
                }),
            }),
            max_failures,
            trip_timeout,
        }
    }

    fn is_call_permitted(&self) -> bool {
        let mut shared = self.inner.shared.lock().unwrap();

        match shared.state {
            State::Closed => true,
            State::HalfOpen(_) => true,
            State::Open(until, delay) => {
                if Instant::now() > until {
                    shared.transit_to_half_open(delay);
                    true
                } else {
                    false
                }
            }
        }
    }

    fn on_error(&self) {
        let mut shared = self.inner.shared.lock().unwrap();
        match shared.state {
            State::Closed => {
                shared.consecutive_failures += 1;
                if shared.consecutive_failures >= self.max_failures {
                    shared.transit_to_open(self.trip_timeout);
                }
            }
            State::HalfOpen(delay_in_half_open) => {
                shared.transit_to_open(delay_in_half_open);
            }
            _ => {}
        }
    }

    fn on_success(&self) {
        let mut shared = self.inner.shared.lock().unwrap();
        if let State::HalfOpen(_) = shared.state {
            shared.transit_to_closed();
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

#[allow(unused_must_use)]
fn main() {

    let circuit_breaker = StateMachine::new(3, Duration::from_secs(10));
    println!("Circuit Breaker has been set with");
    println!("    * 3 as maximum consecutive failures");
    println!("    * 10 seconds as the trip timeout");
    println!("");

    println!("Circuit Breaker is in the initial state, which is closed.");
    // The circuit breaker is in the closed state, so the function
    // will be executed
    let result = circuit_breaker.call(|| request(5));
    println!("Result for request_dice(5): {:?}", result);

    println!("Circuit Breaker is encounting 3 errors in a row ...");
    // The function returns an error 3 times in a row, so the circuit
    // breaker transitions to the open state
    println!("The first one...");
    circuit_breaker.call(|| request(10));
    println!("The second one...");
    circuit_breaker.call(|| request(10));
    println!("The third one...");
    circuit_breaker.call(|| request(10));

    // The circuit breaker is in the open state, so the function is
    // not executed
    let result = circuit_breaker.call(|| request(2));
    println!("Result for request_dice(2): {:?}", result);

    // The circuit breaker is in the half-open state after trip_timeout
    // seconds, so the function will be executed
    println!("Let's have fun by doing nothing in 20 seconds :)");
    println!("...");
    thread::sleep(Duration::from_secs(20));
    let result = circuit_breaker.call(|| request(5));
    println!("Result for request_dice(5): {:?}", result);
    let result = circuit_breaker.call(|| request(6));
    println!("Result for request_dice(6): {:?}", result);
}
