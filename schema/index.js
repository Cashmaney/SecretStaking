import { SecretNetwork, loadSchemas } from '@hackbg/fadroma'

export const schema = loadSchemas(import.meta.url, {
    initMsg:     './init.json',
    queryMsg:    './query_msg.json',
    queryAnswer: './response.json',
    handleMsg:   './handle.json'
})

export default class SecretCash extends SecretNetwork.Contract.withSchema(schema) {

    // query contract status
    // get status () { return this.q.status() }
    //
    // // set the split proportions
    // configure = (config=[]) => this.tx.configure({ config })
    //
    // // claim portions from mgmt and distribute them to recipients
    // vest = () => this.tx.vest()

}