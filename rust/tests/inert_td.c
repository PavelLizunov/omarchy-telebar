/* Synthetic JSON-ABI fixture for exact Rust/QML consumer tests, not TDLib.
 * No credentials, files, devices or network. Parameters and mutations are
 * rejected and counted. Fixed two-chat history exercises the real projection.
 */
#include <json-c/json.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define MAX_EVENTS 256
static char *queue[MAX_EVENTS];
static size_t first, count;
static char *previous;
static int clients;
static json_object *get(json_object *object, const char *key) {
    json_object *value=NULL;
    json_object_object_get_ex(object,key,&value);
    return value;
}
static void publish(int client,const char *body,json_object *extra) {
    if(count>=MAX_EVENTS) abort();
    json_object *value=json_tokener_parse(body);
    if(!value) abort();
    json_object_object_add(value,"@client_id",json_object_new_int(client));
    if(extra)json_object_object_add(value,"@extra",json_object_get(extra));
    queue[(first+count)%MAX_EVENTS]=strdup(json_object_to_json_string_ext(value,JSON_C_TO_STRING_PLAIN));
    if(!queue[(first+count)%MAX_EVENTS])abort();
    count++;json_object_put(value);
}
int td_create_client_id(void) {
    if(++clients>10)abort();
    publish(clients,"{\"@type\":\"updateAuthorizationState\",\"authorization_state\":{\"@type\":\"authorizationStateReady\"}}",NULL);
    publish(clients,"{\"@type\":\"updateConnectionState\",\"state\":{\"@type\":\"connectionStateReady\"}}",NULL);
    publish(clients,"{\"@type\":\"updateUser\",\"user\":{\"@type\":\"user\",\"id\":900,\"first_name\":\"Fixture\",\"last_name\":\"Owner\",\"type\":{\"@type\":\"userTypeRegular\"}}}",NULL);
    publish(clients,"{\"@type\":\"updateNewChat\",\"chat\":{\"@type\":\"chat\",\"id\":101,\"title\":\"Synthetic conversation\",\"type\":{\"@type\":\"chatTypePrivate\",\"user_id\":101},\"positions\":[{\"@type\":\"chatPosition\",\"list\":{\"@type\":\"chatListMain\"},\"order\":\"300\"}],\"last_message\":{\"@type\":\"message\",\"id\":3,\"chat_id\":101,\"date\":1791816000,\"content\":{\"@type\":\"messageText\",\"text\":{\"@type\":\"formattedText\",\"text\":\"Rust feeds the native Main.\",\"entities\":[]}}}}}",NULL);
    return clients;
}
void td_send(int client,const char *text) {
    json_object *query=json_tokener_parse(text);if(!query)abort();
    const char *type=json_object_get_string(get(query,"@type"));json_object *extra=get(query,"@extra");
    if(!type)abort();
    if(strcmp(type,"close")==0){publish(client,"{\"@type\":\"updateAuthorizationState\",\"authorization_state\":{\"@type\":\"authorizationStateClosed\"}}",NULL);}
    else if(strcmp(type,"getOption")==0){publish(client,"{\"@type\":\"optionValueString\",\"value\":\"inert-abi\"}",extra);}
    else if(strcmp(type,"getMe")==0){publish(client,"{\"@type\":\"user\",\"id\":900,\"first_name\":\"Fixture\",\"last_name\":\"Owner\",\"phone_number\":\"\",\"type\":{\"@type\":\"userTypeRegular\"}}",extra);}
    else if(strcmp(type,"loadChats")==0||strcmp(type,"loadActiveStories")==0||strcmp(type,"setOption")==0||strcmp(type,"openChat")==0||strcmp(type,"closeChat")==0){publish(client,"{\"@type\":\"ok\"}",extra);}
    else if(strcmp(type,"getScopeNotificationSettings")==0){publish(client,"{\"@type\":\"scopeNotificationSettings\",\"mute_for\":0,\"show_preview\":true}",extra);}
    else if(strcmp(type,"getChatHistory")==0){publish(client,"{\"@type\":\"messages\",\"total_count\":3,\"messages\":[{\"@type\":\"message\",\"id\":1,\"chat_id\":101,\"date\":1791816000,\"sender_id\":{\"@type\":\"messageSenderUser\",\"user_id\":101},\"content\":{\"@type\":\"messageText\",\"text\":{\"@type\":\"formattedText\",\"text\":\"The native client is connected to an isolated Rust owner.\",\"entities\":[]}}},{\"@type\":\"message\",\"id\":2,\"chat_id\":101,\"date\":1791816060,\"is_outgoing\":true,\"sender_id\":{\"@type\":\"messageSenderUser\",\"user_id\":900},\"content\":{\"@type\":\"messageText\",\"text\":{\"@type\":\"formattedText\",\"text\":\"Synthetic history. No Telegram messages were sent.\",\"entities\":[]}}},{\"@type\":\"message\",\"id\":3,\"chat_id\":101,\"date\":1791816120,\"sender_id\":{\"@type\":\"messageSenderUser\",\"user_id\":101},\"content\":{\"@type\":\"messageText\",\"text\":{\"@type\":\"formattedText\",\"text\":\"Rust feeds the native Main.\",\"entities\":[]}}}]}",extra);}
    else {publish(client,"{\"@type\":\"error\",\"code\":400,\"message\":\"inert native operation refused\"}",extra);}
    json_object_put(query);
}
const char *td_receive(double timeout) {
    (void)timeout;free(previous);previous=NULL;
    if(!count)return NULL;
    previous=queue[first];first=(first+1)%MAX_EVENTS;count--;return previous;
}
const char *td_execute(const char *query) {
    (void)query;return "{\"@type\":\"ok\"}";
}
