;; The .cost-voting contract

;; error codes
(define-constant ERR_PROPOSE 1)

;; cost vote token
(define-fungible-token cost-vote-token)

;; proposal counter
(define-data-var proposal-counter uint u0)

;; max confirmed cost-funciton ID
(define-data-var cost-function-max-id uint u1000)

;; cost-function proposals
(define-map proposals
    ((proposal-id uint))
    ((cost-function-contract principal)
     (cost-function-name (string-utf8 50))
     (function-contract principal)
     (function-name (string-utf8 50))
     (expiration-block-height uint)))

;; confirmed cost-function proposals
(define-map confirmed-proposals
   ((proposal-id uint))
   ((confirmed-proposal
      (optional  
        {  function-contract: principal,
           function-name: (string-ascii 50),
           cost-function-contract: principal,
           cost-function-name: (string-ascii 50),
           confirmed-height: uint }))))

(define-map functions-to-confirmed-ids
   ((function-contract principal) (function-name (string-ascii 50)))
   ((proposal-id uint)))

;; cost-function proposal votes
(define-map proposal-votes ((proposal-id uint)) ((votes uint)))

;; cost-function proposal vetos
(define-map proposal-vetos ((proposal-id uint)) ((vetos uint)))

;; the number of votes a specific principal has committed to a proposal
(define-map principal-proposal-votes ((address principal) (proposal-id uint)) ((votes uint)))

;; getter for cost-function proposals
(define-read-only (get-proposal (proposal-id uint))
    (map-get? proposals { proposal-id: proposal-id }))

;; getter for confirmed cost-function proposals
(define-read-only (get-confirmed-proposal (proposal-id uint))
    (map-get? confirmed-proposals { proposal-id: proposal-id }))

;; getter for cost-function proposal votes
(define-read-only (get-proposal-votes (proposal-id uint))
    (get votes (map-get? proposal-votes { proposal-id: proposal-id })))

;; getter for cost-function proposal vetos
(define-read-only (get-proposal-vetos (proposal-id uint))
    (get vetos (map-get? proposal-vetos { proposal-id: proposal-id })))

;; getter for cost-function proposal votes, for specific principal
(define-read-only (get-principal-votes (address principal) (proposal-id uint))
    (get votes (map-get? principal-proposal-votes { address: address, proposal-id: proposal-id })))

;; Propose cost-functions
(define-public (submit-proposal (function-contract principal)
                                (function-name (string-utf8 50))
                                (cost-function-contract principal)
                                (cost-function-name (string-utf8 50)))
    (begin
        (map-insert proposals { proposal-id: (var-get proposal-counter) }
                              { cost-function-contract: cost-function-contract,
                                cost-function-name: cost-function-name,
                                function-contract: function-contract,
                                function-name: function-name,
                                expiration-block-height: (+ burn-block-height u2016) })
        (map-insert proposal-votes { proposal-id: (var-get proposal-counter) } { votes: u0 })
        (var-set proposal-counter (+ (var-get proposal-counter) u1))
        (ok (- (var-get proposal-counter) u1))))

;; Vote on a proposal
(define-public (vote-proposal (proposal-id uint) (amount uint))
    (let (
        (cur-votes (default-to u0 (get votes (map-get? proposal-votes { proposal-id: proposal-id }))))
        (cur-principal-votes (default-to u0 (get votes (map-get? principal-proposal-votes {
            address: tx-sender,
            proposal-id: proposal-id }))))
        (expiration-block-height (get expiration-block-height (unwrap! (map-get? proposals {
            proposal-id: proposal-id }) (err 1))))
    )
    (if (and
            (< burn-block-height expiration-block-height)
            (is-none (map-get? confirmed-proposals { proposal-id: proposal-id })))
        (begin
            (unwrap! (stx-transfer? amount tx-sender (as-contract tx-sender)) (err 1))
            (unwrap! (ft-mint? cost-vote-token amount tx-sender) (err 1))
            (map-set proposal-votes { proposal-id: proposal-id } { votes: (+ amount cur-votes) })
            (map-set principal-proposal-votes { address: tx-sender, proposal-id: proposal-id}
                                            { votes: (+ amount cur-principal-votes)})
            (ok true))
    (err 1)))
)

;; Withdraw votes
(define-public (withdraw-votes (proposal-id uint) (amount uint))
    (let (
        (cur-votes (default-to u0 (get votes (map-get? proposal-votes { proposal-id: proposal-id }))))
        (cur-principal-votes (default-to u0 (get votes (map-get? principal-proposal-votes {
            address: tx-sender,
            proposal-id: proposal-id }))))
        (expiration-block-height (get expiration-block-height (unwrap! (map-get? proposals {
            proposal-id: proposal-id }) (err 1))))
        (sender tx-sender)
    )
    (if (and
            (>= cur-principal-votes amount)
            (< burn-block-height expiration-block-height)
            (is-none (map-get? confirmed-proposals { proposal-id: proposal-id })))
        (begin
            (unwrap! (as-contract (stx-transfer? amount tx-sender sender)) (err 1))
            (unwrap! (as-contract (ft-transfer? cost-vote-token amount sender tx-sender)) (err 1))
            (map-set proposal-votes { proposal-id: proposal-id } { votes: (- cur-votes amount) })
            (map-set principal-proposal-votes { address: tx-sender, proposal-id: proposal-id }
                                              { votes: (- cur-principal-votes amount) })
            (ok true))
        (err 1)))
)

;; Withdraw STX after vote is over
(define-public (withdraw-after-votes (proposal-id uint))
    (let (
        (cur-principal-votes (default-to u0 (get votes (map-get? principal-proposal-votes {
            address: tx-sender,
            proposal-id: proposal-id }))))
        (expiration-block-height (get expiration-block-height (unwrap! (map-get? proposals {
            proposal-id: proposal-id }) (err 1))))
        (sender tx-sender)
    )
    (if (or
            (> burn-block-height expiration-block-height)
            (is-some (map-get? confirmed-proposals { proposal-id: proposal-id })))
        (begin
            (unwrap! (as-contract (stx-transfer? cur-principal-votes tx-sender sender)) (err 1))
            (unwrap! (as-contract (ft-transfer? cost-vote-token cur-principal-votes sender tx-sender)) (err 1))
            (ok true))
        (err 1)))
)

;; Miner veto
(define-public (veto (proposal-id uint))
    (let (
        (cur-vetos (default-to u0 (get vetos (map-get? proposal-vetos { proposal-id: proposal-id }))))
        (expiration-block-height (get expiration-block-height (unwrap! (map-get? proposals {
            proposal-id: proposal-id }) (err 1))))
    )
    (if (and
            (is-eq contract-caller (unwrap! (get-block-info? miner-address (- block-height u1)) (err 1)))
            (> burn-block-height expiration-block-height)
            (is-none (map-get? confirmed-proposals { proposal-id: proposal-id })))
        (begin
            (map-set proposal-vetos { proposal-id: proposal-id } { vetos: (+ u1 cur-vetos) })
            (ok true)
        )
        (err 1)))
)

;; Confirm proposal has reached required vote count
(define-public (confirm (proposal-id uint))
    (let (
        (votes (default-to u0 (get votes (map-get? proposal-votes { proposal-id: proposal-id }))))
        (vetos (default-to u0 (get vetos (map-get? proposal-vetos { proposal-id: proposal-id }))))
        (expiration-block-height (get expiration-block-height (unwrap! (map-get? proposals {
            proposal-id: proposal-id }) (err 1))))
    )
    (let (
        (blocks-since-proposal (- block-height (- expiration-block-height u2016)))
    )
    (begin
        (if (and
            (>= (/ (* votes u100) stx-liquid-supply) u20)
            (< (/ (* vetos u100) blocks-since-proposal) u80)
        )
        (ok true)
        (ok false))
    )))
)