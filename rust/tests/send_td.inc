/* Explicit TELEBAR_SEND_FIXTURE-only local mutation oracle. No file reads,
 * network, credentials or TDLib. It echoes admitted input paths through the
 * actual JSON ABI and emits pending/settled updates for projection checks.
 */
static int fixture_messages, fixture_profile, fixture_refused, fixture_next=10;
static json_object *fixture_input;
static int fixture_ping_refused;
static int fixture_resources(int client,const char *type,json_object *query,json_object *extra) {
    if(!strcmp(type,"getOption") && get(query,"name") && !strcmp(json_object_get_string(get(query,"name")),"telebarFixtureInput")) {
        json_object *input=get(query,"fixture_file");if(!input || json_object_get_type(input)!=json_type_object)abort();
        if(fixture_input) {json_object_put(fixture_input);} fixture_input=json_object_get(input);publish(client,"{\"@type\":\"ok\"}",extra);return 1;
    }
    if(!strcmp(type,"getOption") && get(query,"name") && !strcmp(json_object_get_string(get(query,"name")),"telebarFixturePingRefuse")) {fixture_ping_refused=1;publish(client,"{\"@type\":\"ok\"}",extra);return 1;}
    if(!strcmp(type,"pingProxy")) {
        if(fixture_ping_refused) {fixture_ping_refused=0;publish(client,"{\"@type\":\"error\",\"code\":400,\"message\":\"synthetic ping refused\"}",extra);}
        else {publish(client,"{\"@type\":\"seconds\",\"seconds\":0.125}",extra);}return 1;
    }
    if(!strcmp(type,"getFile") || !strcmp(type,"downloadFile")) {
        if(!fixture_input) {abort();} publish(client,json_object_to_json_string_ext(fixture_input,JSON_C_TO_STRING_PLAIN),extra);return 1;
    }
    if(!strcmp(type,"getMessage")) {
        if(!fixture_input) {abort();} json_object *m=json_tokener_parse("{\"@type\":\"message\",\"id\":77,\"chat_id\":101,\"content\":{\"@type\":\"messageDocument\",\"document\":{\"@type\":\"document\"}}}");json_object_object_add(get(get(m,"content"),"document"),"document",json_object_get(fixture_input));publish(client,json_object_to_json_string_ext(m,JSON_C_TO_STRING_PLAIN),extra);json_object_put(m);return 1;
    }
    if(!strcmp(type,"openMessageContent")) {publish(client,"{\"@type\":\"ok\"}",extra);return 1;}
    if(!strcmp(type,"getMessageProperties")) {publish(client,"{\"@type\":\"messageProperties\",\"can_be_saved\":true}",extra);return 1;}
    if(!strcmp(type,"getStorageStatisticsFast")) {publish(client,"{\"@type\":\"storageStatisticsFast\",\"files_size\":19,\"file_count\":1,\"database_size\":23,\"log_size\":0}",extra);return 1;}
    if(!strcmp(type,"optimizeStorage")) {publish(client,"{\"@type\":\"storageStatistics\",\"size\":42}",extra);return 1;}
    return 0;
}
static void fixture_publish(int client,json_object *value,json_object *extra) {
    publish(client,json_object_to_json_string_ext(value,JSON_C_TO_STRING_PLAIN),extra);
}
static json_object *fixture_file(json_object *input) {
    json_object *file=json_tokener_parse("{\"@type\":\"file\",\"id\":100,\"size\":1,\"local\":{\"@type\":\"localFile\",\"is_downloading_completed\":true,\"downloaded_size\":1}}");
    json_object *local=get(file,"local"),*path=get(input,"path");
    if(path)json_object_object_add(local,"path",json_object_get(path));
    return file;
}
static json_object *fixture_content(json_object *input) {
    const char *kind=json_object_get_string(get(input,"@type"));
    if(!kind)abort();
    json_object *out=json_object_new_object();
    if(!strcmp(kind,"inputMessageText")) {
        json_object_object_add(out,"@type",json_object_new_string("messageText"));
        json_object_object_add(out,"text",json_object_get(get(input,"text")));return out;
    }
    const char *key,*filekey,*type,*bodytype;
    if(!strcmp(kind,"inputMessagePhoto")) {key="photo";filekey="photo";type="messagePhoto";bodytype="photo";}
    else if(!strcmp(kind,"inputMessageVideo")) {key="video";filekey="video";type="messageVideo";bodytype="video";}
    else if(!strcmp(kind,"inputMessageVideoNote")) {key="video_note";filekey="video";type="messageVideoNote";bodytype="videoNote";}
    else if(!strcmp(kind,"inputMessageVoiceNote")) {key="voice_note";filekey="voice";type="messageVoiceNote";bodytype="voiceNote";}
    else if(!strcmp(kind,"inputMessageDocument")) {key="document";filekey="document";type="messageDocument";bodytype="document";}
    else {json_object_put(out);return NULL;}
    json_object *source=get(input,key),*body=json_object_new_object();
    json_object_object_add(out,"@type",json_object_new_string(type));
    json_object_object_add(body,"@type",json_object_new_string(bodytype));
    for(size_t i=0;i<6;i++) {const char *keys[]={"duration","width","height","length","waveform","file_name"};json_object *v=get(source,keys[i]);if(v)json_object_object_add(body,keys[i],json_object_get(v));}
    if(!strcmp(key,"photo")) {
        json_object *size=json_tokener_parse("{\"@type\":\"photoSize\",\"type\":\"x\",\"width\":12,\"height\":8}"),*sizes=json_object_new_array();
        json_object_object_add(size,"photo",fixture_file(get(source,key)));json_object_array_add(sizes,size);json_object_object_add(body,"sizes",sizes);
    } else {
        const char *localkey=!strcmp(key,"video_note")?"video_note":!strcmp(key,"voice_note")?"voice_note":key;
        json_object_object_add(body,filekey,fixture_file(get(source,localkey)));
    }
    json_object_object_add(out,key,body);json_object *caption=get(input,"caption");if(caption)json_object_object_add(out,"caption",json_object_get(caption));
    return out;
}
static json_object *fixture_message(json_object *query,json_object *content,int id) {
    json_object *m=json_tokener_parse("{\"@type\":\"message\",\"date\":1791816120,\"is_outgoing\":true,\"sender_id\":{\"@type\":\"messageSenderUser\",\"user_id\":900}}");
    json_object_object_add(m,"id",json_object_new_int(id));json_object_object_add(m,"chat_id",json_object_get(get(query,"chat_id")));json_object_object_add(m,"content",content);
    json_object_object_add(m,"sending_state",json_tokener_parse("{\"@type\":\"messageSendingStatePending\"}"));
    return m;
}
static void fixture_settled(int client,json_object *message) {
    int old=json_object_get_int(get(message,"id"));
    json_object *update=json_object_new_object();json_object_object_add(update,"@type",json_object_new_string("updateNewMessage"));json_object_object_add(update,"message",json_object_get(message));fixture_publish(client,update,NULL);json_object_put(update);
    json_object_object_del(message,"sending_state");json_object_object_add(message,"id",json_object_new_int(old+1000));
    update=json_object_new_object();json_object_object_add(update,"@type",json_object_new_string("updateMessageSendSucceeded"));json_object_object_add(update,"old_message_id",json_object_new_int(old));json_object_object_add(update,"message",json_object_get(message));fixture_publish(client,update,NULL);json_object_put(update);
}
static int fixture_send(int client,const char *type,json_object *query,json_object *extra) {
    if(!strcmp(type,"setProfilePhoto")) {fixture_profile++;publish(client,"{\"@type\":\"ok\"}",extra);return 1;}
    if(strcmp(type,"sendMessage") && strcmp(type,"sendMessageAlbum"))return 0;
    json_object *contents=get(query,"input_message_contents"),*one=get(query,"input_message_content");size_t n=contents?json_object_array_length(contents):1;
    if(n==0 || n>10)abort();
    json_object *messages=json_object_new_array();
    for(size_t i=0;i<n;i++) {
        json_object *input=contents?json_object_array_get_idx(contents,i):one,*content=fixture_content(input);
        if(!content)abort();
        json_object *text=get(get(input,"text"),"text");
        if(text && !strcmp(json_object_get_string(text),"fixture-reject")) {fixture_refused++;publish(client,"{\"@type\":\"error\",\"code\":400,\"message\":\"synthetic send refused\"}",extra);json_object_put(messages);return 1;}
        json_object_array_add(messages,fixture_message(query,content,++fixture_next));fixture_messages++;
    }
    if(contents) {json_object *out=json_object_new_object();json_object_object_add(out,"@type",json_object_new_string("messages"));json_object_object_add(out,"messages",json_object_get(messages));fixture_publish(client,out,extra);json_object_put(out);}
    else fixture_publish(client,json_object_array_get_idx(messages,0),extra);
    for(size_t i=0;i<n;i++)fixture_settled(client,json_object_array_get_idx(messages,i));
    json_object_put(messages);return 1;
}
static void fixture_stats(int client,json_object *extra) {
    json_object *out=json_object_new_object();json_object_object_add(out,"@type",json_object_new_string("telebarFixtureStats"));json_object_object_add(out,"messages",json_object_new_int(fixture_messages));json_object_object_add(out,"profile",json_object_new_int(fixture_profile));json_object_object_add(out,"refused",json_object_new_int(fixture_refused));fixture_publish(client,out,extra);json_object_put(out);
}
