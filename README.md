<a name="readme-top"></a>
<br />
<div align="center">

  <h1 align="center">Starrol</h1>

  <p align="center">
</div>



<!-- TABLE OF CONTENTS -->
<!-- <details>
  <summary>Table of Contents</summary>
  <ol>
    <li>
      <a href="#about-the-project">About The Project</a>
      <ul>
        <li><a href="#built-with">Built With</a></li>
      </ul>
    </li>
    <li>
      <a href="#development">Development</a>
      <ul>
        <li><a href="#prerequisites">Prerequisites</a></li>
        <li><a href="#Se">Installation</a></li>
      </ul>
    </li>
    <li><a href="#usage">Usage</a></li>
    <li><a href="#UML">UML</a></li>
    <li><a href="#contributing">Contributing</a></li>
    <li><a href="#license">License</a></li>
    <li><a href="#contact">Contact</a></li>
    <li><a href="#acknowledgments">Acknowledgments</a></li>
  </ol>
</details> -->



<!-- ABOUT THE PROJECT -->
## About The Project
Starrol is a post-quantum, privacy-preserving credential verification framework built on STARKs via the RISC Zero zkVM. It tries to addresses the impending quantum vulnerability of curve-based eID systems by minimizing elliptic curve dependencies and trusted setups. By aggregating issuer credential roots into a unified global Merkle tree, the protocol minimizes linkability between holders and issuing institutions while enabling light-client verification through segmented subtrees.

<p align="right">(<a href="#readme-top">back to top</a>)</p>



### Built With

[![Go][Go]][Go-url]
[![rust][rust]][rust-url]
[![risc0][risc0]][risc0-url]
[![Ethereum][Ethereum]][Ethereum-url]
[![React][React.js]][React-url]
[![wails][wails]][wails-url]

<p align="right">(<a href="#readme-top">back to top</a>)</p>

## Architectural overview 

The system decouples identity issuance from verification by using a zero-knowledge execution environment and a global state aggregator.

### 1. Global Merkle Tree & Batching
Issuers generate local Merkle trees for the credentials they issue and submit only the roots to a central Sequencer. The Sequencer batches these distinct issuer roots into a single, top-level Global Merkle Tree. Because credential membership is proved against this aggregated global root rather than an isolated issuer root, verifiers cannot determine which specific institution issued the credential, breaking issuer-holder linkability.

### 2. Light Client & Subtree Segmentation
To ensure holders don't have to download the entire global state tree, the Merkle tree is segmented into fixed size subtrees (roughly 16k leaves) ($g_1, g_2, \dots, g_n$). Holders operate as light clients and only download the specific sub-tree slice ($g_x$) containing their credential. This bounds bandwidth and local storage requirements while keeping client-side proof construction fast.

### 3. Prover-Side Layer Extraction Optimization
To keep zkVM execution costs low during state updates and revocations, the system avoids generating individual proofs for every sub-tree ($g_1 \dots g_n$). Instead, during verification inside the zkVM, the prover extracts the specific intermediate layer containing all updated subtree roots. Because any leaf verification path passes through higher layers up to the global root, a single aggregated ZK proof is submitted to the Ethereum smart contract to validate all state transitions simultaneously.

<p align="right">(<a href="#readme-top">back to top</a>)</p>
<!-- USAGE EXAMPLES -->



<!-- UML -->
## System Flow & Data Pipeline
 ### 1. Sequence Diagram
<img width="2277" height="1401" alt="hLNVRzis47xNNt580ID16ooodSJrmr0diTEWQPhPwHxsHfCdcuXCUXJb91tsluyYAUWK6LiAp80bYlpkutS_ZlnKM6PikSaJLbgjoiqIJRAM0Y7JsYxrSv8KZr9jcM4RM3B-k3AwLE9Ivzh0kvF9oJIT48J0Z9MvVS1dEzFAth4Dmi42LnhX-yr11-X12_a38HtiNumRpKQpoUoIQYdmpn8LH_Wh0VUR6AEV8lZZS3Au63NJ3sl97nKM" src="https://github.com/user-attachments/assets/4bad8ea1-f829-4f7f-b440-9e28d240e325" />


 ### 2. System Flow
<img width="1096" height="960" alt="Gemini_Generated_Image_r0chxdr0chxdr0ch" src="https://github.com/user-attachments/assets/b182a328-52a8-472e-9d90-ff4b4541ccc3" />

<!-- CONTRIBUTING -->
## Contributing


If you have a suggestion that would make this better, please fork the repo and create a pull request. You can also simply open an issue with the tag "enhancement".
Don't forget to give the project a star! Thanks again!
<p align="right">(<a href="#readme-top">back to top</a>)</p>

<!-- CONTACT -->
## Contact

Suyash - suyashsaraf5@gmail.com

---
Thank You!

<!-- MARKDOWN LINKS & IMAGES -->
<!-- https://www.markdownguide.org/basic-syntax/#reference-style-links -->
[Go]: https://img.shields.io/badge/Go-00ADD8?style=for-the-badge&logo=go&logoColor=white
[Go-url]: https://go.dev/
[React.js]: https://img.shields.io/badge/React-20232A?style=for-the-badge&logo=react&logoColor=61DAFB
[React-url]: https://reactjs.org/
[Ethereum]: https://img.shields.io/badge/Ethereum-3C3C3D?style=for-the-badge&logo=Ethereum&logoColor=white
[Ethereum-url]: https://ethereum.org/
[mongodb]: https://img.shields.io/badge/-MongoDB-13aa52?style=for-the-badge&logo=mongodb&logoColor=white
[mongodb-url]: https://www.mongodb.com/
[wails]: https://img.shields.io/badge/wails-red?style=for-the-badge&logo=wails
[wails-url]: https://wails.io
[risc0]: https://img.shields.io/badge/RISC0-FFC700?style=for-the-badge
[risc0-url]: https://risczero.com/
[rust]: https://img.shields.io/badge/Rust-E57324?style=for-the-badge&logo=rust&logoColor=white
[rust-url]: https://rust-lang.org/
