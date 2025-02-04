use crate::handle_response;
use essential_builder_types::SolutionFailure;
use essential_node_types::register_contract_solution;
use essential_types::{contract::Contract, solution::Solution, ContentAddress};
use reqwest::{Client, ClientBuilder};

/// Client that binds to an Essential builder address.
#[derive(Clone)]
pub struct EssentialBuilderClient {
    /// Async reqwest client to make requests with.
    client: Client,
    /// The url to make requests to.
    url: reqwest::Url,
}

impl EssentialBuilderClient {
    /// Create a new client with the given address.
    pub fn new(addr: String) -> anyhow::Result<Self> {
        let client = ClientBuilder::new().http2_prior_knowledge().build()?;
        let url = reqwest::Url::parse(&addr)?;
        Ok(Self { client, url })
    }

    /// Deploy contract.
    ///
    /// Creates a solution to the contract registry predicate and submits it.
    pub async fn deploy_contract(&self, contract: &Contract) -> anyhow::Result<ContentAddress> {
        let registry_predicate = essential_node_types::BigBang::default().contract_registry;
        let solution = register_contract_solution(registry_predicate, contract)?;
        self.submit_solution(&Solution {
            data: vec![solution],
        })
        .await
    }

    /// Submit solution.
    ///
    /// This allows submitting a solution to be included in an upcoming block.
    /// Once a solution is submitted it is added to the pool.
    /// The block builder runs on a regular loop interval and will include the solution in a block in FIFO order if it satisfies the constraints.
    ///
    /// The block builder is likely to become more sophisticated in the future.
    ///
    /// Note that currently if you submit a solution that conflicts with another solution then whichever solution is submitted first will be included in the block and the other solution will fail. Failed solutions are not retried and will eventually be pruned.
    ///
    /// A solution can conflict with another solution when one solution is built on top of pre-state that the other solution changes. For example if a counter can only increment by 1 and is currently set to 5 then you submit a solution setting it to 6 but another solution is submitted before yours that sets the counter to 6 then your solution will fail to satisfy the constraints.
    /// In fact in this example your solution will never satisfy again unless you update the state mutation to the current count + 1. But to do this you have to resubmit your solution.
    ///
    /// Submitting the same solution twice (even by different user) is idempotent.
    ///
    /// Returns the content address of the submitted solution.
    pub async fn submit_solution(&self, solution: &Solution) -> anyhow::Result<ContentAddress> {
        let url = self.url.join("/submit-solution")?;
        let response = handle_response(self.client.post(url).json(solution).send().await?).await?;
        Ok(response.json::<ContentAddress>().await?)
    }

    /// For solution in the given content address, get the latest solution failures.
    ///
    /// The number of failures returned is limited by the `limit` parameter.
    /// The failures are ordered by block number and solution index in descending order.
    ///
    /// This allows querying the latest failures of a solution.
    /// A solution is either successfully included in a block or it fails with a reason.
    ///
    /// One thing to keep in mind is solutions are not necessarily unique.
    /// It is possible for the same solution to be submitted multiple times.
    /// For example if the counter example also allowed decrementing by 1 then a solution could increment the count from 4 to 5 and another solution could decrement the count from 5 to 4.
    /// Then a solution that increments the count from 4 to 5 could be submitted again.
    /// These two solutions would have the exact same content address.
    /// This results in the same solution hash returning multiple outcomes.
    ///
    /// This might make it difficult to know if it was the solution that you submitted that
    /// was successful or failed. But actually it doesn't really matter because there is no
    /// real ownership over a solution. Remember if two of the same solution are submitted
    /// at the same time then it is as if only one was submitted.
    ///
    /// If you are interested in "has my solution worked" then it probably makes more
    /// sense to query the state of the contract that you were trying to change.
    pub async fn latest_solution_failures(
        &self,
        solution_ca: &ContentAddress,
        limit: u32,
    ) -> anyhow::Result<Vec<SolutionFailure<'static>>> {
        let url = self
            .url
            .join(&format!("/latest_solution_failures/{solution_ca}/{limit}"))?;
        let response = handle_response(self.client.get(url).send().await?).await?;
        Ok(response.json::<Vec<SolutionFailure<'static>>>().await?)
    }
}
