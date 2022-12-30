Rusty Circuit Breaker

What is Circuit Breaker?

I've talked about Circuit Breaker in one of my previous posts, Microservice Governance - Resilience Patterns - Part 2. If you are unfamiliar with its basic concept, please check it out first.
It's crucial to make transmissions between states of Circuit Breaker LSM clear before digging into the implementations. So I will quote some parts about the LSM in my previous post here.

We could use an LSM (Limited State Machine) to illustrate the transmissions between statuses, as below figure.
* State - Closed: The circuit breaker is closed, and the target service can be accessed. The circuit breaker maintains a counter of request failures. If it encounters a failed request, the counter will increase 1.
* State - Open: The circuit breaker is open, and the target service can not be accessed. A request to the target service will fail quickly.
* State - Half-Open: The circuit breaker is half-open. It is allowed to try to access the target service. If the request can be accessed successfully, it means that the service is back to normal. Otherwise, the service is still performing poorly.
